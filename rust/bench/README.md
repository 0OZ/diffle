# Benchmarking the Rust server against the Node server

A black-box comparison: each server is started on the same repository, then driven over HTTP
with `ab` (keep-alive) and a WebSocket probe. Nothing here links against either server.

## Run it

You need `ab` (`apache2-utils`), Node 22+, Python 3, a Rust release build, and a checkout of the
commit you want to compare against, built with `npm run build`:

```sh
cargo build --release --locked --bin diffle                   # in rust/
git -C ~/diffle archive 01e7394 | tar -x -C /tmp/diffle-node   # a pristine commit, not a working copy
ln -s ~/diffle/node_modules /tmp/diffle-node/node_modules && (cd /tmp/diffle-node && npm run build)

BENCH_NODE_BUILD=/tmp/diffle-node rust/bench/run.sh            # 3 rounds, prints the tables
```

`run.sh` generates the `big` repository on first use (3,000 files, 1,600 changed; seeded, so the
same bytes every time). Add a second scenario with a real repository:

```sh
BENCH_REAL_REPO=~/diffle BENCH_REAL_RANGE=main~160..main BENCH_NODE_BUILD=/tmp/diffle-node rust/bench/run.sh
```

| Variable | Meaning | Default |
|---|---|---|
| `BENCH_NODE_BUILD` | built checkout; its `dist/server/main.js` is the Node server, its `dist/client` serves both | required |
| `BENCH_RUST` | the Rust binary | `rust/target/release/diffle` |
| `BENCH_BIG_REPO` / `BENCH_REAL_REPO` | repositories to review | generated `data/big` / unset |
| `BENCH_REAL_RANGE`, `BENCH_REAL_SEARCH` | revspec and search term for the real repository | `HEAD~160..HEAD`, `gamma` |
| `BENCH_MAX_LOAD` | load average under which a run counts as quiet | cores / 4 |
| `BENCH_QUICK` | set to 1 for a smoke test (a twentieth of the requests) | unset |
| `BENCH_OUT`, `BENCH_WORK` | where results and scratch copies go | `results/`, the system temp dir |

## Read it with care

- **Load.** A busy machine measures its other jobs. Before each run `bench.py` waits up to ten
  minutes for the load average to drop under `BENCH_MAX_LOAD`; if it never does, the run is marked
  not quiet and `aggregate.py` leaves it out (`--all` keeps it). Close builds and dev servers first.
- **One client.** `ab` is a single process, so the biggest numbers (about 150,000 req/s) partly
  measure `ab` itself. The ratios matter more than the absolutes.
- **Small samples.** The search and batch-patch rows use 8 and 150 requests, because Node needs
  seconds per search. Their ratios are indicative only.
- **The Node baseline must be a clean commit.** A working copy with uncommitted changes in
  `src/server` is not upstream.
- **Edit to push** is measured through the file watcher for the working-tree scenario only; a range
  review watches refs, not the worktree.

## Differential run: `compare.py`

`compare.py <repo> [revspec args]` replays the calls the client makes against both servers, each on
a fresh copy of `<repo>`, and prints what differs: status codes, response shapes and values,
headers, the patch text, and what a page hears on the WebSocket (after an edit, a new file, a
deletion and a malformed frame). It also records each server's stderr and probes whether a
`--output=` revspec reaches git. Error bodies are compared by shape, not wording.

**The report names files and URLs of the repository it ran on.** When that repository is private,
keep the report out of the diffle checkout and out of anything public; quote counts only.
