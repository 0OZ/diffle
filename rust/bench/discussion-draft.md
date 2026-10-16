# Before posting (delete this block)

This is a draft for the upstream issue Moritz asked for. Nothing here has been posted.

1. **Run the benchmark yourself** on a quiet machine, then paste the tables and fill in the method line. From `rust/`:
   `cargo build --release --locked --bin diffle`, build a pristine Node checkout as the README in `rust/bench/` shows, then
   `BENCH_NODE_BUILD=/tmp/diffle-node rust/bench/run.sh`. It generates the big repository itself, runs 3 rounds with alternating
   order, waits for a quiet machine before each run and drops runs taken under load.
2. **Make the linked code match the numbers.** The issue links `https://github.com/0OZ/diffle/tree/main/rust`. Fork `main` is
   still `fa8122d`: it lacks the `TCP_NODELAY` fix (without it, static files were several times slower than Node), the `app/run.rs`
   move and `rust/bench/`. Commit and push those to the fork first, or the numbers describe code nobody can see.
3. **Check the claims** under "What the prototype does and does not do" and the line counts (about 17,000 lines of source and
   9,000 of tests in `rust/diffle/src`, against 6,800 in `src/server`) against the code you are about to link.
4. **Decide on the mentions.** The draft @-mentions Moritz and Pavel; both will be notified.
5. **Post it yourself, or ask for it.** `gh issue create --repo moritzwilksch/diffle --title "Discussion: a Rust server for diffle, and the design choices to settle first" --body-file <this file, minus this block>`
   posts as your account. Your rule is no upstream contributions until #322 is decided; Moritz's invitation is the exception, so
   it is your call.

---

## Discussion: a Rust server for diffle, and the design choices to settle before any code

Not a PR. [@moritzwilksch](https://github.com/moritzwilksch) suggested opening an issue with numbers and agreeing on the design first, and [@pavelzw](https://github.com/pavelzw) has Rust experience and ideas for crates, so I'd like your input before anything is merged.

### Why look at this

I built a prototype of a server in Rust that speaks the existing HTTP/WebSocket contract (`src/shared/protocol.ts`), so the current client runs against it unchanged. Two things motivated it:

- **Speed and memory** on big reviews (numbers below).
- **A server you can run somewhere else**, for example in a container or on a remote machine behind an SSH forward, with the browser on your laptop. A single static binary with no Node runtime makes that easy. (The prototype ships a `docker compose` file for exactly this.)

It lives in my fork under `rust/` (`https://github.com/0OZ/diffle/tree/main/rust`). I'm not proposing to merge that tree: it is far too big to review as one PR. It exists so we can talk about real numbers and a real layout.

### Benchmarks

**Method.** Each server is started on the same repository, one at a time, and driven with ApacheBench (`ab -k`, keep-alive) plus a WebSocket probe that times an edit through the file watcher to the pushed snapshot. Node is a build of the unmodified upstream commit `01e7394`; Rust is a release build. Two repositories: `big`, a generated repository of 3,000 files with 1,600 changed in the working tree (the generator is seeded, so everyone gets the same bytes), and `real`, a clone of this repository reviewing `main~160..main` (298 changed files). [FILL IN: number of rounds, load average at the start, CPU and OS.] The tables show the median over rounds with the min–max in brackets. The harness is in `rust/bench` and its README has the one command to reproduce them.

Read the numbers with these in mind:

- The Rust prototype does less: no language servers and no GitHub yet. The comparison covers the endpoints both implement.
- `ab` is one process, so the largest absolute numbers partly measure `ab`. The ratios matter more.
- The search and batch-patch rows use small samples (Node needs seconds per search). Treat their ratios as indicative.
- Batch patches and search are where Rust is closest to Node, because both spawn `git` for each request.

[FILL IN: paste the output of `rust/bench/run.sh` here, one table per scenario.]

### What the prototype does and does not do

Does: working-tree reviews (staged and untracked files included) and revspec reviews (ranges and commits, and focusing one commit of a range); snapshot, patch, file, image, search, threads (comments), viewed marks, config, mode switching; the file watcher with debounced pushes over the WebSocket; the Host/Origin guard; serving the built client; the `config` subcommands. 556 unit tests plus an integration test, `clippy -D warnings`, `unsafe_code = "forbid"`, and `unwrap`/`expect`/`panic` denied outside tests.

Does not (yet): language servers (the routes answer with a status the client already handles), GitHub and pull-request modes, range-diff, interdiff and pair modes (501), recorded iterations, `--lsp`, `--timing`, `--dev` and the `lsp` subcommands. Validation error wording differs from the zod messages.

Size: about 17,000 lines of source plus 9,000 of tests, against 6,800 lines in `src/server`. Part of that is a layered layout (below); I'd like to discuss whether that trade is right for this project. Two implementations to keep in sync is a real maintenance cost, and it is the first question below.

### Questions

For each one I've put what the prototype does today. Please push back on any of them.

1. **Do you want a second server at all?** Options: (a) Rust eventually replaces the Node server, (b) both live and Rust is opt-in, (c) it stays outside this repository. The rest depends on this.
2. **Where does it live?** `rust/` in this repository (today), or its own repository. In-tree means CI for another toolchain on Linux, macOS and Windows, x64 and arm64, and a say in the release process (the changelog and version are derived from PR titles today).
3. **How do we keep the two in sync?** `src/shared/protocol.ts` is the only contract and the Rust side mirrors it by hand. Options: keep it by hand plus tests; generate JSON Schema from the zod schemas and check the Rust types against it; or move the contract to a neutral schema both sides generate from. My lean is a conformance gate: let the existing Playwright e2e suite target any server URL and run it against both. That needs no Rust at all and would be a useful first PR on its own.
4. **Crate layout.** Today it is one crate with `app/` (wiring only), `kernel/`, `workers/` (watcher, HTTP server) and `domains/` (review, changes, comments, revisions, repository, settings), and no sub-crates. [@pavelzw](https://github.com/pavelzw), which crates did you have in mind? Splitting out the git layer, the wire types or the LSP bridge are the candidates I see.
5. **Git access.** The prototype shells out to `git` with explicit argv, like `GitRepo.ts`, so behaviour matches exactly. The alternative is `gix` or `git2`. Spawning costs about 50 ms of CPU per batch-patch or search request today, which is why those are the endpoints where Rust is only even with Node.
6. **Dependencies.** `tokio`, `axum` 0.8 (with `ws`), `tower-http`, `notify` 8, `clap` 4, `serde`/`serde_json`, `thiserror`, `tracing`, `rustix`, `uuid`. Anything you'd rather avoid? Current minimum Rust version is 1.97.
7. **Language servers.** The Node `LspBridge` owns process and JSON-RPC lifecycle. A Rust port could add a supervisor that restarts a crashed server and tells the client why one is missing (the "not on PATH" cases). Deferred; I'd do it after 1 to 3 are settled.
8. **GitHub.** `@octokit/core` today; `octocrab` or plain `reqwest` in Rust. Also deferred.
9. **Distribution.** `build:sea` produces a Node single-executable. A Rust binary per platform could ship the same way esbuild and biome do (platform packages via `optionalDependencies`), or as release assets only. What do you want users to install?
10. **Remote mode.** There is no login today; access control is loopback plus an SSH forward. Running a server on another machine raises the question of a token. Out of scope here, but it shapes the answer to 1.

### If the answer to 1 is yes: how I'd land it

Small PRs, each reviewable on its own, none of them changing the default server:

1. The e2e suite can target an external server URL (no Rust).
2. Rust skeleton: workspace, CI job, `kernel`, `/api/config`, static files and the WebSocket.
3. One domain per PR from there: repository and revisions, then changes (snapshot, patch), comments, settings, the watcher.
4. Language servers, then GitHub, then the remaining modes.

If the answer is "not in this repository", that is fine too; the numbers above stand either way and I'd keep it in the fork.
