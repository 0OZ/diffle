#!/usr/bin/env python3
"""Differential run: the same client calls against the Node and the Rust server, then the differences.

usage: compare.py <repo> [revspec args...] [--skip-all-patch] [--json FILE]

<repo> is copied afresh for each server, so state files and the edits of the socket probe never
touch it. Needs BENCH_NODE_BUILD and BENCH_RUST like bench.py (see README.md).
"""
import gzip, json, os, re, shutil, subprocess, sys, tempfile, threading, time
import http.client
from urllib.parse import urlsplit

import jdiff, scenario_read, scenario_write
from report import print_report

HERE = os.path.dirname(os.path.abspath(__file__))
WORK = os.environ.get('BENCH_WORK') or os.path.join(tempfile.gettempdir(), 'diffle-bench')
NODE_BUILD = os.environ.get('BENCH_NODE_BUILD') or ''
RUST_BIN = os.environ.get('BENCH_RUST') or os.path.join(HERE, '..', 'target', 'release', 'diffle')
START_TIMEOUT_S = 180
REQUEST_TIMEOUT_S = 300
COMMON = ['--no-open', '-p', '0', '--no-lsp']
MAX_KEPT_BODY = 64 * 1024 * 1024


class Reply:
    def __init__(self, status, headers, body):
        self.status, self.headers, self.body = status, headers, body
        self._json = self._parsed = None

    def json(self):
        if not self._parsed:
            self._parsed = True
            try:
                self._json = json.loads(self.body)
            except ValueError:
                self._json = None
        return self._json

    def text(self):
        return self.body.decode('utf-8', 'replace')


class Probe:
    """Sends requests to one server and logs every reply in order."""

    def __init__(self, base):
        parts = urlsplit(base)
        self.host, self.port, self.log = parts.hostname, parts.port, []

    def __call__(self, name, method, path, body=None, raw=None, headers=None):
        data = raw if raw is not None else (json.dumps(body).encode() if body is not None else None)
        sent = dict(headers or {})
        if body is not None:
            sent.setdefault('Content-Type', 'application/json')
        started = time.perf_counter()
        conn = http.client.HTTPConnection(self.host, self.port, timeout=REQUEST_TIMEOUT_S)
        try:
            conn.request(method, path, body=data, headers=sent)
            resp = conn.getresponse()
            reply = Reply(resp.status, {k.lower(): v for k, v in resp.getheaders()}, resp.read())
            reply.size = len(reply.body)  # on the wire
            if reply.headers.get('content-encoding') == 'gzip':
                reply.body = gzip.decompress(reply.body)  # compare what it says, not how it was packed
        except (OSError, http.client.HTTPException) as error:
            reply = Reply(0, {}, repr(error).encode())  # status 0: nothing came back
        finally:
            conn.close()
        reply.ms = (time.perf_counter() - started) * 1000
        reply.size = getattr(reply, 'size', len(reply.body))
        if reply.size > MAX_KEPT_BODY:
            reply.body = reply.body[:MAX_KEPT_BODY]
        self.log.append((name, method, path, reply))
        return reply


def start(kind, run_dir, args):
    cmd = ['node', os.path.join(NODE_BUILD, 'dist', 'server', 'main.js')] if kind == 'node' else \
        [RUST_BIN, '--client-dir', os.path.join(NODE_BUILD, 'dist', 'client')]
    env = dict(os.environ, XDG_CONFIG_HOME=os.path.join(WORK, f'compare-config-{kind}'))
    shutil.rmtree(env['XDG_CONFIG_HOME'], ignore_errors=True)
    proc = subprocess.Popen(cmd + COMMON + args, cwd=run_dir, env=env, stderr=subprocess.PIPE, stdout=subprocess.DEVNULL, text=True)
    log = open(os.path.join(WORK, f'compare-{kind}.err'), 'w')
    found = threading.Event()
    url = []

    def pump():
        for line in proc.stderr:
            log.write(line)
            log.flush()
            m = re.search(r'running at (http://\S+)', line)
            if m and not url:
                url.append(m.group(1).rstrip('/').replace('0.0.0.0', '127.0.0.1'))
                found.set()
    threading.Thread(target=pump, daemon=True).start()
    if not found.wait(START_TIMEOUT_S):
        proc.kill()
        sys.exit(f'compare.py: the {kind} server printed no URL; see {log.name}')
    return proc, url[0], log.name


def rss_mb(pid):
    try:
        with open(f'/proc/{pid}/status') as f:
            return next(int(line.split()[1]) / 1024 for line in f if line.startswith('VmRSS'))
    except (OSError, StopIteration):
        return None


def run_server(kind, source, args, opts):
    run_dir = os.path.join(WORK, 'compare-run')
    shutil.rmtree(run_dir, ignore_errors=True)
    os.makedirs(WORK, exist_ok=True)
    subprocess.run(['cp', '-a', source, run_dir], check=True)
    proc, url, errfile = start(kind, run_dir, args)
    probe, ctx = Probe(url), {'skip_all_patch': opts['skip_all_patch']}
    began = time.perf_counter()
    scenario_read.reads(probe, ctx)
    scenario_write.comments(probe, ctx)
    scenario_write.viewed(probe, ctx)
    scenario_write.config(probe, ctx)
    scenario_write.modes(probe, ctx, WORK)
    ws = subprocess.run(['node', os.path.join(HERE, 'wscompare.mjs'), url, run_dir], capture_output=True, text=True)
    try:
        socket_probe = json.loads(ws.stdout)
    except ValueError:
        socket_probe = {'error': (ws.stdout + ws.stderr)[-300:]}
    result = dict(kind=kind, log=probe.log, ctx=ctx, ws=socket_probe, seconds=time.perf_counter() - began,
                  rss_mb=rss_mb(proc.pid), errfile=errfile, alive=proc.poll() is None)
    proc.terminate()
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
    return result


def main():
    argv = sys.argv[1:]
    opts = {'skip_all_patch': '--skip-all-patch' in argv}
    out = argv[argv.index('--json') + 1] if '--json' in argv else None
    plain = [a for i, a in enumerate(argv) if not a.startswith('--') and not (i and argv[i - 1] == '--json')]
    if not plain or not os.path.isdir(plain[0]):
        sys.exit(__doc__)
    if not os.path.isfile(os.path.join(NODE_BUILD, 'dist', 'server', 'main.js')):
        sys.exit('compare.py: BENCH_NODE_BUILD must name a checkout built with `npm run build`')
    source, args = os.path.abspath(plain[0]), plain[1:]
    runs = {kind: run_server(kind, source, args, opts) for kind in ('node', 'rust')}
    print_report(runs, source, args, jdiff)
    if out:
        with open(out, 'w') as f:
            json.dump({k: {'seconds': v['seconds'], 'rss_mb': v['rss_mb'], 'ws': v['ws'],
                           'steps': [[n, m, p, r.status, r.size, round(r.ms, 1)] for n, m, p, r in v['log']]}
                       for k, v in runs.items()}, f)


main()
