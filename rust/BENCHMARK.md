# Benchmarking the Rust server

Two questions, two tools. Both live in [`bench/`](bench/) and drive a real server over HTTP and
WebSocket; nothing links against the server's code.

| Question | Tool | Runs where |
|---|---|---|
| Is the Rust server faster than the Node one, and does it answer the same? | `bench/run.sh`, `bench/compare.py` | by hand, on a quiet machine |
| Did this change make the Rust server slower? | `bench/regression/run.py` | CI, on every pull request that touches `rust/` |

## What is measured

On a generated repository (`big`: 3,000 files, 1,600 of them changed, the same bytes every time),
or on a real repository you name:

- requests per second and latency for the static page, config, snapshot, one patch, a batch of 200
  patches, every patch, a file's contents, threads, and search;
- time from start to listening and to the first snapshot;
- memory after the start and after the load, and CPU for the whole run, Git included;
- time from editing a file to the pushed snapshot on the socket (working-tree reviews only).

## The regression gate

`.github/workflows/rust-bench.yml` builds the pull request's base and head on one runner, runs
`bench.py big rust` five times for each in alternating order, and compares medians. A metric is a
regression only when all three hold:

1. the median is worse by more than 20%,
2. every head run is worse than every base run, otherwise the row says `noisy` and passes,
3. the change exceeds an absolute floor (0.5 req/s, 5 ms, 5 MB, 0.25 s), so 2 ms to 4 ms is not "twice as slow".

Requests that fail where the base had none also fail the job. The table is in the job summary and
the raw runs are the `rust-bench` artifact. Exit status: 0 clean, 1 regression, 2 the benchmark broke.

```sh
cargo build --release --locked --bin diffle                       # in rust/, at the head
python3 bench/regression/run.py --base /path/to/base/diffle --head target/release/diffle
python3 -m unittest discover -v -s bench/regression               # the verdict logic
```

To gate another number, add it to `metrics()` in `bench/regression/verdict.py` with a test.

## Read it with care

- **A shared runner hides small regressions.** The gate catches changes of tens of percent. For a
  few percent, run the full comparison on a quiet machine and look at the spread, not the median.
- **Load decides results.** `bench.py` records the load average at the start of each run. On a busy
  machine the same binary can differ by a factor of two to five between runs.
- **The page is a stub in the gate.** `static index` measures the server, not the client bundle.
- **Search and batch rows are small samples** (8 and 150 requests), because they are slow.

## Your own repositories

`bench/compare.py` and the `real` scenario print file paths and request URLs of the repository they
ran on. Run them on a copy of a private repository, keep the output out of this checkout, and quote
only counts and timings anywhere public.

Setup, every variable, and the Node baseline: [`bench/README.md`](bench/README.md).
