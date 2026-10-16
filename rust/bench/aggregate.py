#!/usr/bin/env python3
"""usage: aggregate.py [results-dir] [--all]

Median of the <scenario>-<server>-<round>.json runs as markdown, with the min–max spread.
Runs that started on a busy machine (bench.py marks them quiet_at_start=false) are left out
unless --all is given: a benchmark taken under someone else's load measures the load.
"""
import glob, json, os, statistics as st, sys

args = [a for a in sys.argv[1:] if not a.startswith('--')]
keep_noisy = '--all' in sys.argv
directory = args[0] if args else 'results'
med = st.median


def load(scenario, server):
    runs, dropped = [], 0
    for path in sorted(glob.glob(os.path.join(directory, f'{scenario}-{server}-*.json'))):
        try:
            run = json.load(open(path))
        except Exception:
            continue
        if 'endpoints' not in run:
            continue
        if run.get('quiet_at_start') is False and not keep_noisy:
            dropped += 1
        else:
            runs.append(run)
    return runs, dropped


def fmt(x, digits=0):
    return f'{x:,.{digits}f}'


def rate(x):
    return fmt(x, 1 if x < 10 else 0)


def spread(values, digits=0):
    return f'{fmt(min(values), digits)}–{fmt(max(values), digits)}'


for scenario in ('big', 'real'):
    (rust, rust_dropped), (node, node_dropped) = load(scenario, 'rust'), load(scenario, 'node')
    if not rust or not node:
        if rust_dropped or node_dropped or rust or node:
            print(f'## {scenario}: not enough quiet runs (rust {len(rust)}, node {len(node)}; '
                  f'dropped for load: rust {rust_dropped}, node {node_dropped}). Rerun on a quiet machine, or pass --all.\n')
        continue
    loads = [r['load1_before'] for r in rust + node]
    print(f"## {scenario}: {rust[0]['changed_files']} changed files; {len(rust)} Rust and {len(node)} Node runs "
          f"(load average at start {spread(loads, 1)} on {rust[0]['cores']} cores; "
          f'{rust_dropped + node_dropped} run(s) dropped for load)\n')
    print('| endpoint | response | Rust req/s (min–max) | Node req/s (min–max) | ratio | Rust p50 / p99 ms | Node p50 / p99 ms |')
    print('|---|---|---|---|---|---|---|')
    for i, e in enumerate(rust[0]['endpoints']):
        r = [run['endpoints'][i] for run in rust]
        n = [run['endpoints'][i] for run in node]
        rr, nr = med([x['rps'] or 0 for x in r]), med([x['rps'] or 0 for x in n])
        ratio = f'{rr / nr:.1f}×' if nr else 'n/a'
        size = f"{e['bytes'] / 1024:,.0f} KB" if e['bytes'] >= 1024 else f"{e['bytes']} B"
        bad = ' ⚠' if any(x['failed'] or x['non2xx'] for x in r + n) else ''
        print(f"| {e['name']} | {size} | {rate(rr)} ({spread([x['rps'] or 0 for x in r])}) | {rate(nr)} ({spread([x['rps'] or 0 for x in n])}) | {ratio}{bad} | "
              f"{fmt(med([x['p50'] or 0 for x in r]))} / {fmt(med([x['p99'] or 0 for x in r]))} | {fmt(med([x['p50'] or 0 for x in n]))} / {fmt(med([x['p99'] or 0 for x in n]))} |")
    print('\n⚠ = `ab` counted failed or non-2xx responses in at least one run.\n')
    print('| | Rust | Node |')
    print('|---|---|---|')
    for key, label in (('listening_ms', 'start → listening (ms)'), ('ready_ms', 'start → first snapshot (ms)'),
                       ('rss_idle_mb', 'RSS idle (MB)'), ('rss_after_load_mb', 'RSS after load (MB)'),
                       ('cpu_total_s', 'CPU for the whole run, git included (s)')):
        a, b = [x[key] for x in rust], [x[key] for x in node]
        print(f'| {label} | {fmt(med(a), 1)} ({spread(a, 1)}) | {fmt(med(b), 1)} ({spread(b, 1)}) |')

    def push(runs):
        vals = [r['edit_to_push']['single_ms']['median'] for r in runs
                if isinstance(r.get('edit_to_push'), dict) and r['edit_to_push'].get('single_ms', {}).get('median') is not None]
        return f'{fmt(med(vals))} ({spread(vals)})' if vals else 'n/a'
    print(f'| edit → pushed snapshot, median (ms) | {push(rust)} | {push(node)} |')
    print()
