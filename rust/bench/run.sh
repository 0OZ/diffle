#!/bin/sh
# usage: BENCH_NODE_BUILD=/path/to/built/checkout ./run.sh [rounds]   (default 3)
# One server at a time, so runs never compete for CPU; the order alternates per round so
# neither server always goes first. The 'real' scenario runs when BENCH_REAL_REPO is set.
set -eu
cd "$(dirname "$0")"
rounds=${1:-3}
out=${BENCH_OUT:-results}
mkdir -p "$out"
rm -f "$out"/*.json "$out"/*.err

[ -d data/big ] || python3 make-big-repo.py data/big

scenarios=big
if [ -n "${BENCH_REAL_REPO:-}" ]; then scenarios="big real"; fi

round=1
while [ "$round" -le "$rounds" ]; do
  for scenario in $scenarios; do
    if [ $((round % 2)) -eq 1 ]; then order="rust node"; else order="node rust"; fi
    for server in $order; do
      echo "round $round: $scenario $server (load $(cut -d' ' -f1 /proc/loadavg))"
      python3 bench.py "$scenario" "$server" ${BENCH_QUICK:+quick} \
        > "$out/$scenario-$server-$round.json" 2> "$out/$scenario-$server-$round.err"
    done
  done
  round=$((round + 1))
done
python3 aggregate.py "$out"
