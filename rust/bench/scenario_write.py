"""The calls that change state: comments, viewed marks, config, mode switches, reload.

Everything runs on a throwaway copy of the repository, so the state files each server keeps under
`.git/diffle/` never touch a repository anyone cares about.
"""
import os
from urllib.parse import quote

GHOST = '00000000-0000-7000-8000-000000000000'  # an id no thread has
LONG_BODY = 'y' * 200_000


def thread_with(threads, text):
    return next((t for t in threads if t['messages'][0]['body'] == text), None) if isinstance(threads, list) else None


def comments(call, ctx):
    samples = ctx['samples']
    target = samples.get('modified') or samples.get('added')
    if target is None:
        return
    path = target['path']
    line = {'path': path, 'side': 'new', 'startLine': 1, 'endLine': 1}
    made = call('thread create (line)', 'POST', '/api/threads', body={**line, 'body': 'first'}).json()
    thread = thread_with(made, 'first')
    call('thread create (file)', 'POST', '/api/threads', body={'path': path, 'body': 'about the file'})
    call('thread create (old side)', 'POST', '/api/threads', body={**line, 'side': 'old', 'body': 'old side'})
    call('threads list', 'GET', '/api/threads')
    call('threads list for path', 'GET', f'/api/threads?path={quote(path)}')
    call('threads export', 'GET', '/api/threads/export?state=open')
    if thread:
        tid = thread['id']
        replied = call('reply', 'POST', f'/api/threads/{tid}/replies', body={'body': 'a reply'}).json()
        mid = replied['messages'][-1]['id'] if isinstance(replied, dict) and replied.get('messages') else GHOST
        call('message export', 'GET', f'/api/threads/{tid}/messages/{mid}/export')
        call('message edit', 'PATCH', f'/api/threads/{tid}/messages/{mid}', body={'body': 'edited'})
        call('message edit empty', 'PATCH', f'/api/threads/{tid}/messages/{mid}', body={'body': ''})
        call('resolve', 'PUT', f'/api/threads/{tid}/resolved', body={'resolved': True})
        call('threads resolved', 'GET', '/api/threads?state=resolved')
        call('unresolve', 'PUT', f'/api/threads/{tid}/resolved', body={'resolved': False})
        call('message delete (reply)', 'DELETE', f'/api/threads/{tid}/messages/{mid}')
        call('message delete again', 'DELETE', f'/api/threads/{tid}/messages/{mid}')
        first = call('thread get after', 'GET', '/api/threads').json()
        survivor = thread_with(first, 'first')
        if survivor:
            call('message delete (only)', 'DELETE', f'/api/threads/{tid}/messages/{survivor["messages"][0]["id"]}')
        call('thread delete', 'DELETE', f'/api/threads/{tid}')
    invalid(call, line, path)
    call('threads delete stale', 'DELETE', '/api/threads?stale=1')
    call('threads clear', 'DELETE', '/api/threads')
    call('threads list after clear', 'GET', '/api/threads')


def invalid(call, line, path):
    call('create empty body', 'POST', '/api/threads', body={**line, 'body': '  '})
    call('create end<start', 'POST', '/api/threads', body={**line, 'startLine': 5, 'endLine': 2, 'body': 'x'})
    call('create side without line', 'POST', '/api/threads', body={'path': path, 'side': 'new', 'body': 'x'})
    call('create unknown path', 'POST', '/api/threads', body={'path': 'no/such/file.txt', 'body': 'x'})
    call('create line beyond file', 'POST', '/api/threads', body={**line, 'startLine': 10**7, 'endLine': 10**7, 'body': 'x'})
    call('create zero line', 'POST', '/api/threads', body={**line, 'startLine': 0, 'endLine': 0, 'body': 'x'})
    call('create huge body', 'POST', '/api/threads', body={**line, 'body': LONG_BODY})
    call('create extra fields', 'POST', '/api/threads', body={**line, 'body': 'x', 'bogus': 1})
    call('create not json', 'POST', '/api/threads', raw=b'{', headers={'Content-Type': 'application/json'})
    call('reply ghost', 'POST', f'/api/threads/{GHOST}/replies', body={'body': 'x'})
    call('resolve ghost', 'PUT', f'/api/threads/{GHOST}/resolved', body={'resolved': True})
    call('resolve not bool', 'PUT', f'/api/threads/{GHOST}/resolved', body={'resolved': 'yes'})
    call('delete ghost thread', 'DELETE', f'/api/threads/{GHOST}')
    call('edit ghost message', 'PATCH', f'/api/threads/{GHOST}/messages/{GHOST}', body={'body': 'x'})
    call('export ghost message', 'GET', f'/api/threads/{GHOST}/messages/{GHOST}/export')


def viewed(call, ctx):
    changed = ctx['snapshot']['changed']
    if not changed:
        return
    f = changed[0]
    call('viewed mark', 'PUT', '/api/viewed', body={'path': f['path'], 'blob': f['blob'], 'viewed': True})
    call('viewed list', 'GET', '/api/viewed')
    call('viewed bulk', 'PUT', '/api/viewed/bulk',
         body={'entries': [{'path': c['path'], 'blob': c['blob'], 'viewed': True} for c in changed[:50]]})
    call('viewed unmark', 'PUT', '/api/viewed', body={'path': f['path'], 'blob': f['blob'], 'viewed': False})
    call('viewed unknown path', 'PUT', '/api/viewed', body={'path': 'no/such', 'blob': 'abc', 'viewed': True})
    call('viewed empty path', 'PUT', '/api/viewed', body={'path': '', 'blob': 'abc', 'viewed': True})
    call('viewed bad body', 'PUT', '/api/viewed', body={'path': f['path']})
    call('viewed bulk 100k entries', 'PUT', '/api/viewed/bulk',
         body={'entries': [{'path': f'p{i}', 'blob': 'b', 'viewed': True} for i in range(100_000)]})
    call('viewed list after', 'GET', '/api/viewed')


def config(call, ctx):
    original = call('config before', 'GET', '/api/config').json()
    call('config context 7', 'PUT', '/api/config', body={'contextLines': 7})
    call('snapshot after context', 'GET', '/api/snapshot')
    call('config auto-viewed', 'PUT', '/api/config', body={'autoViewed': ['*.lock', 'dist/**']})
    call('config follow refs', 'PUT', '/api/config', body={'followRefs': 'never'})
    call('config negative context', 'PUT', '/api/config', body={'contextLines': -1})
    call('config huge context', 'PUT', '/api/config', body={'contextLines': 10**9})
    call('config float context', 'PUT', '/api/config', body={'contextLines': 2.5})
    call('config bad follow', 'PUT', '/api/config', body={'followRefs': 'sometimes'})
    call('config unknown key', 'PUT', '/api/config', body={'lspCommands': {'rust': 'x'}, 'nope': 1})
    call('config empty', 'PUT', '/api/config', body={})
    if isinstance(original, dict):
        restore = {k: original[k] for k in ('autoViewed', 'contextLines', 'followRefs') if k in original}
        call('config restore', 'PUT', '/api/config', body=restore)


def modes(call, ctx, work):
    snap = ctx['snapshot']
    call('mode working', 'POST', '/api/mode', body={'kind': 'working'})
    call('snapshot working', 'GET', '/api/snapshot')
    call('mode revspec HEAD~3..HEAD', 'POST', '/api/mode', body={'kind': 'revspec', 'args': ['HEAD~3..HEAD']})
    after = call('snapshot range', 'GET', '/api/snapshot').json()
    commits = (after or {}).get('commits', {}).get('list', []) if isinstance(after, dict) else []
    if commits:
        call('mode focus commit', 'POST', '/api/mode', body={'kind': 'focus', 'commit': commits[-1].get('sha')})
        call('mode focus range', 'POST', '/api/mode', body={'kind': 'focus', 'commit': None})
    call('mode focus worktree', 'POST', '/api/mode', body={'kind': 'focus', 'commit': 'worktree'})
    call('mode revspec HEAD', 'POST', '/api/mode', body={'kind': 'revspec', 'args': ['HEAD']})
    call('mode revspec a..b..c', 'POST', '/api/mode', body={'kind': 'revspec', 'args': ['HEAD~1..HEAD~2..HEAD']})
    call('mode revspec unknown ref', 'POST', '/api/mode', body={'kind': 'revspec', 'args': ['no-such-ref..HEAD']})
    call('mode revspec empty', 'POST', '/api/mode', body={'kind': 'revspec', 'args': []})
    call('mode revspec empty string', 'POST', '/api/mode', body={'kind': 'revspec', 'args': ['']})
    call('mode revspec many args', 'POST', '/api/mode', body={'kind': 'revspec', 'args': ['HEAD'] * 5000})
    poc = os.path.join(work, 'flag-injection-proof')
    call('mode revspec --output flag', 'POST', '/api/mode', body={'kind': 'revspec', 'args': [f'--output={poc}', 'HEAD~1..HEAD']})
    ctx['flag_injection_wrote'] = os.path.exists(poc)
    call('mode revspec -- path', 'POST', '/api/mode', body={'kind': 'revspec', 'args': ['HEAD~1..HEAD', '--', '/etc/passwd']})
    call('mode unknown kind', 'POST', '/api/mode', body={'kind': 'sideways'})
    call('mode pr', 'POST', '/api/mode', body={'kind': 'pr'})
    call('mode interdiff', 'POST', '/api/mode', body={'kind': 'interdiff', 'from': 1, 'to': 2})
    call('reload', 'POST', '/api/reload')
    call('iterations clear', 'DELETE', '/api/iterations')
    call('snapshot at end', 'GET', '/api/snapshot')
    ctx['mode_restored'] = snap['mode']
