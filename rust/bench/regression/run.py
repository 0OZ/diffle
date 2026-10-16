#!/usr/bin/env python3
"""Benchmark two builds of the Rust server on the same machine and say whether the second got slower.

usage: run.py --base BIN --head BIN [--rounds 5] [--threshold 0.2] [--out DIR] [--summary FILE]

Runs `bench.py big rust` alternately (base, head, head, base, ...) so drift in the machine's
load hits both builds alike, then judges the medians with verdict.py. Prints the table; with
--summary also appends it there (GitHub's $GITHUB_STEP_SUMMARY). Exit 0: no regression,
1: a regression, 2: the benchmark itself failed.
"""
import argparse, json, os, subprocess, sys

import verdict

HERE = os.path.dirname(os.path.abspath(__file__))
BENCH = os.path.join(HERE, '..')
EXIT_REGRESSION, EXIT_BROKEN = 1, 2
# Both builds share the machine, so its load is not a reason to wait: the order alternates instead.
ANY_LOAD = '1000000'


def one_run(binary, out_file, work):
    env = dict(os.environ, BENCH_RUST=os.path.abspath(binary), BENCH_MAX_LOAD=ANY_LOAD, BENCH_WORK=work)
    env.pop('BENCH_NODE_BUILD', None)
    with open(out_file, 'w') as out:
        subprocess.run([sys.executable, os.path.join(BENCH, 'bench.py'), 'big', 'rust'], env=env,
                       stdout=out, stderr=sys.stderr, check=False)
    try:
        with open(out_file) as f:
            return json.load(f)
    except ValueError:
        return {'error': 'bench.py printed no result', 'stderr': ''}


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('--base', required=True)
    parser.add_argument('--head', required=True)
    parser.add_argument('--rounds', type=int, default=5)
    parser.add_argument('--threshold', type=float, default=0.2)
    parser.add_argument('--out', default=os.path.join(BENCH, 'results', 'regression'))
    parser.add_argument('--summary')
    args = parser.parse_args()
    os.makedirs(args.out, exist_ok=True)
    work = os.path.abspath(os.path.join(args.out, 'work'))
    if not os.path.isdir(os.path.join(BENCH, 'data', 'big')):
        subprocess.run([sys.executable, os.path.join(BENCH, 'make-big-repo.py'), os.path.join(BENCH, 'data', 'big')], check=True)

    runs = {'base': [], 'head': []}
    for round_ in range(1, args.rounds + 1):
        order = ('base', 'head') if round_ % 2 else ('head', 'base')
        for label in order:
            print(f'round {round_}: {label}', file=sys.stderr, flush=True)
            runs[label].append(one_run(getattr(args, label), os.path.join(args.out, f'{label}-{round_}.json'), work))
    try:
        result = verdict.judge(runs['base'], runs['head'], args.threshold)
    except verdict.BenchFailed as error:
        print(f'benchmark failed: {error}', file=sys.stderr)
        return EXIT_BROKEN
    table = verdict.render(result, args.threshold)
    print(table)
    if args.summary:
        with open(args.summary, 'a') as f:
            f.write('## Rust server benchmark\n\n' + table)
    return EXIT_REGRESSION if result.regressions else 0


sys.exit(main())
