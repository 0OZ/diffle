#!/usr/bin/env python3
"""Black-box benchmark of the Rust and Node review servers on identical repositories.

usage: bench.py <scenario> <server> [quick]
  scenario: big | real      server: rust | node
Prints one JSON document. Each run works on a fresh copy of the pristine repository.
Configuration is by environment; README.md lists it.
"""
import json, os, re, shutil, signal, subprocess, sys, tempfile, time, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
WORK = os.environ.get('BENCH_WORK') or os.path.join(tempfile.gettempdir(), 'diffle-bench')
NODE_BUILD = os.environ.get('BENCH_NODE_BUILD') or ''
CORES = os.cpu_count() or 1
QUIET_LOAD = float(os.environ.get('BENCH_MAX_LOAD') or CORES / 4)
QUIET_WAIT_S = 600
CLK = os.sysconf('SC_CLK_TCK')

RUST = [os.environ.get('BENCH_RUST') or os.path.join(HERE, '..', 'target', 'release', 'diffle'),
        '--client-dir', os.path.join(NODE_BUILD, 'dist', 'client')]
NODE = ['node', os.path.join(NODE_BUILD, 'dist', 'server', 'main.js')]
COMMON = ['--no-open', '-p', '0', '--no-lsp']

SCENARIOS = {
    'big': dict(repo=os.environ.get('BENCH_BIG_REPO') or os.path.join(HERE, 'data', 'big'),
                args=[], one='src/mod010/file0.rs', search='gamma'),
    'real': dict(repo=os.environ.get('BENCH_REAL_REPO') or '',
                 args=[os.environ.get('BENCH_REAL_RANGE') or 'HEAD~160..HEAD'],
                 one='package.json', search=os.environ.get('BENCH_REAL_SEARCH') or 'gamma'),
}


def proc_stats(pid):
    with open(f'/proc/{pid}/stat') as f:
        fields = f.read().rsplit(')', 1)[1].split()
    cpu = sum(int(fields[i]) for i in (11, 12, 13, 14)) / CLK  # utime stime cutime cstime
    rss = 0
    with open(f'/proc/{pid}/status') as f:
        for line in f:
            if line.startswith('VmRSS'):
                rss = int(line.split()[1]) / 1024
    return cpu, rss


def get(url, data=None):
    t = time.perf_counter()
    req = urllib.request.Request(url, data=data, headers={'content-type': 'application/json'} if data else {})
    with urllib.request.urlopen(req, timeout=120) as r:
        body = r.read()
    return (time.perf_counter() - t) * 1000, len(body), body


def ab(url, n, c, post_file=None):
    cmd = ['ab', '-q', '-k', '-r', '-s', '300', '-n', str(n), '-c', str(c)]
    if post_file:
        cmd += ['-p', post_file, '-T', 'application/json']
    out = subprocess.run(cmd + [url], capture_output=True, text=True).stdout

    def num(pattern):
        m = re.search(pattern, out)
        return float(m.group(1)) if m else None
    pct = {p: num(rf'\n\s*{p}%\s+(\d+)') for p in (50, 95, 99)}
    return dict(rps=num(r'Requests per second:\s+([\d.]+)'), mean_ms=num(r'Time per request:\s+([\d.]+) \[ms\] \(mean\)'),
                p50=pct[50], p95=pct[95], p99=pct[99], max=num(r'\n\s*100%\s+(\d+)'),
                failed=num(r'Failed requests:\s+(\d+)'), non2xx=num(r'Non-2xx responses:\s+(\d+)') or 0)


def wait_until_quiet():
    """Other jobs on a shared machine ruin a benchmark; wait for a quiet minute, else say so."""
    deadline = time.monotonic() + QUIET_WAIT_S
    while os.getloadavg()[0] > QUIET_LOAD and time.monotonic() < deadline:
        time.sleep(10)
    return os.getloadavg()[0] <= QUIET_LOAD


def main():
    scenario, server = sys.argv[1], sys.argv[2]
    quick = len(sys.argv) > 3
    sc = SCENARIOS[scenario]
    if not sc['repo'] or not os.path.isdir(sc['repo']):
        sys.exit(f"bench.py: no repository for '{scenario}' (looked at {sc['repo'] or 'BENCH_REAL_REPO, unset'})")
    if not os.path.isfile(NODE[1]):
        sys.exit(f'bench.py: BENCH_NODE_BUILD must name a checkout built with `npm run build`; {NODE[1]} is missing')
    quiet = wait_until_quiet()
    load_before = os.getloadavg()[0]
    run_dir = os.path.join(WORK, f'run-{scenario}-{server}')
    shutil.rmtree(run_dir, ignore_errors=True)
    os.makedirs(WORK, exist_ok=True)
    subprocess.run(['cp', '-a', sc['repo'], run_dir], check=True)
    env = dict(os.environ, XDG_CONFIG_HOME=os.path.join(WORK, f'config-{server}'))
    cmd = (RUST if server == 'rust' else NODE) + COMMON + sc['args']
    t0 = time.perf_counter()
    proc = subprocess.Popen(cmd, cwd=run_dir, env=env, stderr=subprocess.PIPE, stdout=subprocess.DEVNULL, text=True)
    url = None
    for line in proc.stderr:
        m = re.search(r'running at (http://\S+)', line)
        if m:
            url = m.group(1).rstrip('/').replace('0.0.0.0', '127.0.0.1')
            break
    if url is None:
        print(json.dumps({'error': 'no url printed', 'stderr': proc.stderr.read()[-400:]}))
        return
    listening_ms = (time.perf_counter() - t0) * 1000
    # Keep draining stderr so the server never blocks on a full pipe.
    subprocess.Popen(['cat'], stdin=proc.stderr, stdout=subprocess.DEVNULL)
    ready_ms = None
    while ready_ms is None and time.perf_counter() - t0 < 120:
        try:
            get(f'{url}/api/snapshot')
            ready_ms = (time.perf_counter() - t0) * 1000
        except Exception:
            time.sleep(0.005)
    pid = proc.pid
    cpu0, rss_idle = proc_stats(pid)
    snap = json.loads(get(f'{url}/api/snapshot')[2])
    changed = [f['path'] for f in snap['changed']]
    res = dict(scenario=scenario, server=server, changed_files=len(changed), listening_ms=round(listening_ms),
               ready_ms=round(ready_ms), rss_idle_mb=round(rss_idle, 1), cpu_startup_s=round(cpu0, 2),
               cores=CORES, load1_before=round(load_before, 2), quiet_at_start=quiet)

    one = sc['one'] if sc['one'] in changed else changed[0]
    batch = json.dumps({'paths': changed[:200]})
    batch_file = os.path.join(WORK, 'batch.json')
    with open(batch_file, 'w') as f:
        f.write(batch)
    endpoints = [
        ('static index', '/', None, 20000, 32),
        ('config', '/api/config', None, 20000, 32),
        ('snapshot', '/api/snapshot', None, 3000, 32),
        ('patch one file', f'/api/patch?path={one}', None, 8000, 32),
        ('file (new side)', f'/api/file?path={one}&rev=new', None, 5000, 32),
        ('threads', '/api/threads', None, 20000, 32),
        ('patch batch of 200', '/api/patch', batch_file, 150, 8),
        ('patch all files', '/api/patch', 'GET', 40, 4),
        ('search', f'/api/search?q={sc["search"]}', None, 8, 2),
    ]
    out = []
    for name, path, extra, n, c in endpoints:
        if quick:
            n = max(10, n // 20)
        post = extra if extra and extra != 'GET' else None
        try:
            cold_ms, size, _ = get(url + path, batch.encode() if post else None)
        except Exception:
            cold_ms, size = None, 0
        c0, _ = proc_stats(pid)
        r = ab(url + path, n, c, post)
        c1, rss = proc_stats(pid)
        r.update(name=name, n=n, c=c, bytes=size, cold_ms=round(cold_ms, 1) if cold_ms else None,
                 cpu_ms_per_req=round((c1 - c0) * 1000 / n, 3), rss_mb=round(rss, 1))
        out.append(r)
    res['endpoints'] = out
    _, res['rss_after_load_mb'] = [round(x, 1) for x in proc_stats(pid)]

    # Edit -> pushed snapshot over the socket, through the file watcher.
    if sc['args']:
        res['edit_to_push'] = 'n/a: a range review watches refs, not the worktree'
    else:
        ws = subprocess.run(['node', os.path.join(HERE, 'wspush.mjs'), url, run_dir, changed[1] if len(changed) > 1 else changed[0],
                             '5' if quick else '12', '60' if scenario == 'big' else '10'], capture_output=True, text=True)
        try:
            res['edit_to_push'] = json.loads(ws.stdout)
        except Exception:
            res['edit_to_push'] = {'error': (ws.stdout + ws.stderr)[-300:]}
    cpu_end, _ = proc_stats(pid)
    res['cpu_total_s'] = round(cpu_end, 2)
    res['load1_after'] = round(os.getloadavg()[0], 2)
    proc.send_signal(signal.SIGTERM)
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
    print(json.dumps(res))


main()
