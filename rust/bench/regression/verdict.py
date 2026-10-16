"""Did a change make the server slower? Compares two sets of `bench.py` runs of one scenario.

A shared CI machine is noisy, so a verdict needs two things: the median differs by more than
the threshold, and every run of the change lies beyond every run of the base. A median that
differs while the runs overlap is reported as noise, never failed and never celebrated. Differences below a metric's
floor (2 ms to 4 ms is "100% slower") never count.
"""
import statistics
from dataclasses import dataclass

MIN_RUNS = 3
HIGHER, LOWER = 'higher is better', 'lower is better'
MINUS = '−'


class BenchFailed(Exception):
    """The benchmark itself did not produce numbers; that is not a pass."""


@dataclass(frozen=True)
class Kind:
    better: str
    floor: float


REQS, TIME_MS, MEMORY_MB, CPU_S = Kind(HIGHER, 0.5), Kind(LOWER, 5.0), Kind(LOWER, 5.0), Kind(LOWER, 0.25)


@dataclass(frozen=True)
class Row:
    name: str
    base: float
    head: float
    worse_by: float | None  # fraction; negative is better; None when the base was zero
    status: str  # ok | improved | noisy | regression


@dataclass(frozen=True)
class Verdict:
    rows: list
    runs: tuple

    @property
    def regressions(self):
        return [row for row in self.rows if row.status == 'regression']


def metrics(run):
    """Every gated number of one run: name -> (value, kind)."""
    found = {f"{e['name']}: req/s": (e['rps'] or 0.0, REQS) for e in run['endpoints']}
    found['start → first snapshot (ms)'] = (run['ready_ms'], TIME_MS)
    found['RSS after load (MB)'] = (run['rss_after_load_mb'], MEMORY_MB)
    found['CPU for the whole run (s)'] = (run['cpu_total_s'], CPU_S)
    pushed = run.get('edit_to_push')
    if isinstance(pushed, dict) and pushed.get('single_ms', {}).get('median') is not None:
        found['edit → pushed snapshot (ms)'] = (pushed['single_ms']['median'], TIME_MS)
    return found


def failures(run):
    """Failed or non-2xx responses per endpoint, as `ab` counted them."""
    return {f"{e['name']}: failed requests": (e.get('failed') or 0) + (e.get('non2xx') or 0)
            for e in run['endpoints']}


def checked(runs, label):
    if len(runs) < MIN_RUNS:
        raise BenchFailed(f'{label}: {len(runs)} run(s); need at least {MIN_RUNS} to tell noise from change')
    for run in runs:
        if 'endpoints' not in run:
            raise BenchFailed(f"{label}: a run failed: {run.get('error', 'no endpoints')} {run.get('stderr', '')}".strip())


def worse_by(base, head, kind):
    if base == 0:
        return 0.0 if kind.better == HIGHER or head == 0 else None
    return (base - head) / base if kind.better == HIGHER else (head - base) / base


def apart(base_runs, head_runs, kind, slower):
    """Whether every head run lies on the worse (or better) side of every base run."""
    head_below, head_above = max(head_runs) < min(base_runs), min(head_runs) > max(base_runs)
    return head_below if (kind.better == HIGHER) == slower else head_above


def status_of(base, head, base_runs, head_runs, kind, threshold):
    worse = worse_by(base, head, kind)
    if abs(head - base) < kind.floor or worse is not None and abs(worse) <= threshold:
        return 'ok'
    slower = worse is None or worse > 0
    if not apart(base_runs, head_runs, kind, slower):
        return 'noisy'
    return 'regression' if slower else 'improved'


def judge(base_runs, head_runs, threshold=0.2):
    checked(base_runs, 'base')
    checked(head_runs, 'head')
    base_all, head_all = [metrics(r) for r in base_runs], [metrics(r) for r in head_runs]
    rows = []
    for name, (_, kind) in base_all[0].items():
        if not all(name in m for m in base_all + head_all):
            continue
        base_values, head_values = [m[name][0] for m in base_all], [m[name][0] for m in head_all]
        base, head = statistics.median(base_values), statistics.median(head_values)
        rows.append(Row(name, base, head, worse_by(base, head, kind),
                        status_of(base, head, base_values, head_values, kind, threshold)))
    base_failed, head_failed = [failures(r) for r in base_runs], [failures(r) for r in head_runs]
    for name in base_failed[0]:
        before, after = max(f.get(name, 0) for f in base_failed), max(f.get(name, 0) for f in head_failed)
        if after > before:
            rows.append(Row(name, before, after, None, 'regression'))
    return Verdict(rows, (len(base_runs), len(head_runs)))


VERDICT_LABELS = {'ok': '', 'improved': '🟢 better', 'noisy': '〰 noisy', 'regression': '❌ regression'}


def number(value):
    return f'{value:,.0f}' if value >= 10 else f'{value:,.1f}'


def change(row):
    if row.base == 0:
        return 'new'
    delta = (row.head - row.base) / row.base
    return f'{"+" if delta >= 0 else MINUS}{abs(delta):.1%}'


def render(result, threshold):
    """The comparison as a markdown table with a one-line summary above it."""
    count = len(result.regressions)
    base_runs, head_runs = result.runs
    if count:
        summary = f'**{count} regression{"s" if count != 1 else ""}** beyond {threshold:.0%}'
    else:
        summary = f'**No regression** beyond {threshold:.0%}'
    lines = [f'{summary} (medians of {base_runs} base and {head_runs} head runs). A regression is a median '
             'worse than the threshold with every head run worse than every base run; overlapping runs '
             'are shown as noise.', '',
             '| metric | base | head | change | |', '|---|---:|---:|---:|---|']
    lines += [f'| {r.name} | {number(r.base)} | {number(r.head)} | {change(r)} | {VERDICT_LABELS[r.status]} |'
              for r in result.rows]
    return '\n'.join(lines) + '\n'
