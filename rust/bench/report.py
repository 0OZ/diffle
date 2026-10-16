"""The report of a differential run (compare.py): what differs between the Node and Rust servers."""
import os
import re
from collections import Counter

ID_OR_TIME = re.compile(r'[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}|\d{4}-\d\d-\d\dT[\d:.]+Z?')
MAX_GROUPS_PER_STEP = 10
SLOWEST = 8
LOG_PATTERN = re.compile(r'panic|ERROR|WARN|error|failed', re.I)


def media(reply):
    return reply.headers.get('content-type', '').split(';')[0].strip()


def spaced(value):
    """`text/plain;charset=UTF-8` and `text/plain; charset=utf-8` are the same header."""
    return re.sub(r';\s*', '; ', value.lower()) if value else value


def shape(value):
    """An error body with its wording removed: keys and types stay, strings go."""
    if isinstance(value, dict):
        return {k: shape(v) for k, v in value.items()}
    if isinstance(value, list):
        return [shape(v) for v in value]
    return '<str>' if isinstance(value, str) else value


def paired(runs):
    """Steps of both servers matched by name and occurrence."""
    seen = {kind: Counter() for kind in runs}
    table = {}
    for kind, run in runs.items():
        for name, method, path, reply in run['log']:
            seen[kind][name] += 1
            table.setdefault((name, seen[kind][name]), {'method': method, 'path': path})[kind] = reply
    return [(key[0], v) for key, v in table.items()]


def line_diff(a, b):
    la, lb = a.splitlines(), b.splitlines()
    differing = sum(1 for x, y in zip(la, lb) if x != y) + abs(len(la) - len(lb))
    first = next((i for i, (x, y) in enumerate(zip(la, lb)) if x != y), min(len(la), len(lb)))
    return differing, first, (la[first] if first < len(la) else '<end>')[:120], (lb[first] if first < len(lb) else '<end>')[:120]


def compare_bodies(name, node, rust, jdiff, ids, out):
    nj, rj = node.json(), rust.json()
    if nj is None and rj is None:
        a, b = ID_OR_TIME.sub('<x>', node.text()), ID_OR_TIME.sub('<x>', rust.text())
        if a != b:
            count, at, la, lb = line_diff(a, b)
            out['text'].append((name, node.size, rust.size, count, at, la, lb))
        return
    if (nj is None) != (rj is None):
        out['kind'].append((name, node.status, media(node), rust.status, media(rust)))
        return
    nn, rn = jdiff.normalize(nj, ids['node']), jdiff.normalize(rj, ids['rust'])
    if node.status >= 400:
        nn, rn = shape(nn), shape(rn)
    findings = jdiff.Findings()
    jdiff.compare(nn, rn, findings)
    if findings:
        out['json'].append((name, findings))


def sections(runs, jdiff):
    out = {'status': [], 'json': [], 'text': [], 'kind': [], 'headers': Counter()}
    ids = {'node': {}, 'rust': {}}
    for name, v in paired(runs):
        node, rust = v.get('node'), v.get('rust')
        if node is None or rust is None:
            out['status'].append((name, node.status if node else 'not run', rust.status if rust else 'not run', v['method'], v['path']))
            continue
        if node.status != rust.status:
            out['status'].append((name, node.status, rust.status, v['method'], v['path']))
            continue
        for header in ('content-type', 'content-encoding', 'cache-control'):
            a, b = spaced(node.headers.get(header)), spaced(rust.headers.get(header))
            if a != b:
                out['headers'][(header, a, b)] += 1
        compare_bodies(name, node, rust, jdiff, ids, out)
    return out


def log_summary(run):
    with open(run['errfile'], errors='replace') as f:
        lines = f.read().splitlines()
    return len(lines), [line[:160] for line in lines if LOG_PATTERN.search(line)][:4]


def print_report(runs, source, args, jdiff):
    print(f'# Node vs Rust on `{os.path.basename(source)}` {" ".join(args) or "(working tree)"}\n')
    print('| | Node | Rust |\n|---|---|---|')
    steps = {k: len(r['log']) for k, r in runs.items()}
    print(f"| requests | {steps['node']} | {steps['rust']} |")
    print(f"| seconds for the scenario | {runs['node']['seconds']:.0f} | {runs['rust']['seconds']:.0f} |")
    print(f"| RSS at the end (MB) | {runs['node']['rss_mb'] or 0:.0f} | {runs['rust']['rss_mb'] or 0:.0f} |")
    print(f"| still running at the end | {runs['node']['alive']} | {runs['rust']['alive']} |")
    print(f"| `--output=` flag reached git | {runs['node']['ctx'].get('flag_injection_wrote')} | {runs['rust']['ctx'].get('flag_injection_wrote')} |")
    for kind in ('node', 'rust'):
        total, hits = log_summary(runs[kind])
        print(f'\n{kind} stderr: {total} lines' + ''.join(f'\n- `{h}`' for h in hits))
    found = sections(runs, jdiff)

    print(f"\n## Different status ({len(found['status'])})\n\n| step | request | Node | Rust |\n|---|---|---|---|")
    for name, a, b, method, path in found['status']:
        print(f'| {name} | `{method} {path[:70]}` | {a} | {b} |')
    print(f"\n## Different body ({len(found['json'])} steps)")
    for name, findings in found['json']:
        print(f'\n**{name}**')
        for (kind, path), g in sorted(findings.groups.items(), key=lambda kv: -kv[1]['count'])[:MAX_GROUPS_PER_STEP]:
            node_sample, rust_sample = g['samples'][0]
            print(f'- {kind} `{path}` ×{g["count"]}: node {node_sample} / rust {rust_sample}')
    print(f"\n## Different text ({len(found['text'])})\n\n| step | bytes (node / rust) | lines differing | first difference |\n|---|---|---|---|")
    for name, na, nb, count, at, la, lb in found['text']:
        print(f'| {name} | {na:,} / {nb:,} | {count} | line {at}: `{la}` vs `{lb}` |')
    print(f"\n## JSON on one side only ({len(found['kind'])})")
    for name, sn, mn, sr, mr in found['kind']:
        print(f'- {name}: node {sn} {mn!r}, rust {sr} {mr!r}')
    print('\n## Headers\n')
    for (header, a, b), count in found['headers'].most_common():
        print(f'- `{header}`: node `{a}` / rust `{b}` on {count} steps')

    print('\n## Slowest requests (ms; noisy on a shared machine)\n\n| step | Node ms | Rust ms | size |\n|---|---|---|---|')
    slow = sorted(paired(runs), key=lambda p: -max(p[1][k].ms for k in ('node', 'rust') if k in p[1]))[:SLOWEST]
    for name, v in slow:
        n, r = v.get('node'), v.get('rust')
        print(f"| {name} | {n.ms if n else 0:,.0f} | {r.ms if r else 0:,.0f} | {max(v[k].size for k in ('node', 'rust') if k in v):,} |")
    print('\n## Socket: what a page hears\n')
    for phase in ('initial', 'afterEdit', 'afterCreate', 'afterDelete', 'afterBadFrame', 'stillOpen', 'error'):
        a, b = runs['node']['ws'].get(phase), runs['rust']['ws'].get(phase)
        if a is not None or b is not None:
            print(f"- {phase}: {'same' if a == b else 'DIFFERENT'}" + ('' if a == b else f'\n  - node `{a}`\n  - rust `{b}`'))
