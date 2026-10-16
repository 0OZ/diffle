"""The read-only calls: static files, every GET the client makes, and hostile or malformed input.

`call(name, method, path, body=None, raw=None, headers=None)` sends one request and records the
reply; it returns the reply so a later request can use what an earlier one answered.
"""
import re
from urllib.parse import urlencode

BATCH_LIMIT = 200  # PatchRequestSchema: paths.max(200)
BIG_BODY = b'x' * (5 * 1024 * 1024)
TRAVERSALS = ('../../../etc/passwd', '/etc/passwd', '..%2f..%2fetc%2fpasswd', 'src/../../etc/passwd', '.', '', '\x00')
SEARCHES = (
    {'q': 'TODO', 'i': '1'},
    {'q': 'import', 'scope': 'diff', 'content': 'diff', 'word': '1'},
    {'q': 'func|const', 're': '1', 'scope': 'diff'},
    {'q': '(', 're': '1'},
    {'q': ''},
    {'q': 'TODO(', 'scope': 'repo', 'content': 'full'},
)


def pick_samples(snapshot):
    changed = snapshot['changed']

    def first(test):
        return next((f for f in changed if test(f)), None)

    odd = first(lambda f: any(ord(c) > 127 or c in ' "\'\\#%&+' for c in f['path']))
    picks = {
        'modified': first(lambda f: f['status'] == 'M' and not f['binary']),
        'added': first(lambda f: f['status'] == 'A' and not f['binary']),
        'deleted': first(lambda f: f['status'] == 'D'),
        'renamed': first(lambda f: f['status'] in ('R', 'C')),
        'binary': first(lambda f: f['binary']),
        'generated': first(lambda f: f['generated']),
        'submodule': first(lambda f: f.get('submodule')),
        'biggest': max(changed, key=lambda f: f['additions'] + f['deletions'], default=None),
        'odd name': odd,
    }
    return {label: f for label, f in picks.items() if f}


def q(**params):
    return urlencode(params)


def statics(call):
    index = call('static /', 'GET', '/')
    call('static / gzip', 'GET', '/', headers={'Accept-Encoding': 'gzip'})
    call('static /index.html', 'GET', '/index.html')
    call('static spa route', 'GET', '/some/deep/route')
    call('static missing asset', 'GET', '/assets/nope.js')
    asset = re.search(rb'/assets/[^"\']+\.js', index.body)
    if asset:
        call('static asset', 'GET', asset.group(0).decode())
        call('static asset gzip', 'GET', asset.group(0).decode(), headers={'Accept-Encoding': 'gzip'})
        call('static asset if-none-match', 'GET', asset.group(0).decode(), headers={'If-None-Match': '"x"'})


def reads(call, ctx):
    statics(call)
    snap = call('snapshot', 'GET', '/api/snapshot').json()
    ctx['snapshot'] = snap
    samples = pick_samples(snap)
    ctx['samples'] = samples
    for endpoint in ('config', 'moved', 'refs', 'github', 'lsp/status', 'viewed', 'threads', 'threads/export'):
        call(endpoint, 'GET', f'/api/{endpoint}')
    for state in ('open', 'resolved', 'all', 'bogus'):
        call(f'threads state={state}', 'GET', f'/api/threads?{q(state=state)}')
    for old, new in (('0', '1'), ('1', '0'), ('0', '999999'), ('x', '1'), ('-1', '1')):
        call(f'last-commits {old},{new}', 'GET', f'/api/last-commits-preview?{q(oldOffset=old, newOffset=new)}')
    call('last-commits missing', 'GET', '/api/last-commits-preview')

    for label, f in samples.items():
        call(f'patch {label}', 'GET', f'/api/patch?{q(path=f["path"])}')
        for side in ('new', 'old'):
            call(f'file {label} {side}', 'GET', f'/api/file?{q(path=f["path"], rev=side)}')
        if f['binary']:
            for side, blob in (('new', f['blob']), ('old', f['oldBlob'])):
                call(f'image {label} {side}', 'GET', f'/api/image?{q(path=f["path"], rev=side, key=blob)}')
    paths = [f['path'] for f in snap['changed']]
    call('patch batch', 'POST', '/api/patch', body={'paths': paths[:BATCH_LIMIT]})
    call('patch batch over limit', 'POST', '/api/patch', body={'paths': (paths * 2 + ['x'] * BATCH_LIMIT)[:BATCH_LIMIT + 1]})
    call('patch batch empty', 'POST', '/api/patch', body={'paths': []})
    call('patch batch unknown path', 'POST', '/api/patch', body={'paths': ['no/such/file.txt']})
    call('patch batch duplicates', 'POST', '/api/patch', body={'paths': paths[:1] * 3})
    if not ctx.get('skip_all_patch'):
        call('patch all', 'GET', '/api/patch')
    for params in SEARCHES:
        call(f'search {params}', 'GET', f'/api/search?{q(**params)}')
    if 'modified' in samples:
        call('search scope=file', 'GET', f'/api/search?{q(q="a", scope="file", path=samples["modified"]["path"])}')
    errors(call, samples, snap)


def errors(call, samples, snap):
    for bad in TRAVERSALS:
        call(f'file path {bad!r}', 'GET', f'/api/file?{q(path=bad, rev="new")}')
        call(f'patch path {bad!r}', 'GET', f'/api/patch?{q(path=bad)}')
        call(f'image path {bad!r}', 'GET', f'/api/image?{q(path=bad, rev="new", key="x")}')
    call('file raw %2e%2e', 'GET', '/api/file?path=%2e%2e%2f%2e%2e%2fetc%2fpasswd&rev=new')
    call('file no params', 'GET', '/api/file')
    call('file bad rev', 'GET', f'/api/file?{q(path=snap["tree"][0] if snap["tree"] else "x", rev="sideways")}')
    if snap['tree']:
        call('file directory', 'GET', f'/api/file?{q(path=snap["tree"][0].rsplit("/", 1)[0], rev="new")}')
        call('file unchanged tracked', 'GET', f'/api/file?{q(path=snap["tree"][-1], rev="new")}')
    call('patch unknown path', 'GET', f'/api/patch?{q(path="no/such/file.txt")}')
    call('unknown api route', 'GET', '/api/nope')
    call('unknown api route POST', 'POST', '/api/nope', body={})
    call('method not allowed PUT snapshot', 'PUT', '/api/snapshot', body={})
    call('method not allowed POST config', 'POST', '/api/config', body={})
    call('HEAD snapshot', 'HEAD', '/api/snapshot')
    call('OPTIONS snapshot', 'OPTIONS', '/api/snapshot')
    call('trailing slash', 'GET', '/api/config/')
    call('POST not json', 'POST', '/api/patch', raw=b'not json', headers={'Content-Type': 'application/json'})
    call('POST wrong content-type', 'POST', '/api/patch', raw=b'{"paths":[]}', headers={'Content-Type': 'text/plain'})
    call('POST no content-type', 'POST', '/api/patch', raw=b'{"paths":[]}')
    call('POST 5 MB body', 'POST', '/api/patch', raw=BIG_BODY, headers={'Content-Type': 'application/json'})
    call('foreign Host', 'GET', '/api/snapshot', headers={'Host': 'evil.example'})
    call('foreign Origin', 'GET', '/api/snapshot', headers={'Origin': 'http://evil.example'})
    call('foreign Origin POST', 'POST', '/api/reload', headers={'Origin': 'http://evil.example'})
