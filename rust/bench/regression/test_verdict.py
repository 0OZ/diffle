"""Tests for verdict.py: when does a change count as slower?  Run: python3 -m unittest"""
import unittest

import verdict


def run(rps=1000.0, ready_ms=100.0, rss_mb=50.0, cpu_s=5.0, push_ms=40.0, failed=0, non2xx=0):
    """One bench.py result with two endpoints; `rps` applies to the first."""
    return {
        'endpoints': [
            {'name': 'snapshot', 'rps': rps, 'failed': failed, 'non2xx': non2xx},
            {'name': 'search', 'rps': 3.0, 'failed': 0, 'non2xx': 0},
        ],
        'ready_ms': ready_ms, 'rss_after_load_mb': rss_mb, 'cpu_total_s': cpu_s,
        'edit_to_push': {'single_ms': {'median': push_ms}},
    }


def rows_by_name(result):
    return {row.name: row for row in result.rows}


class Judging(unittest.TestCase):
    def test_identical_runs_regress_nothing(self):
        result = verdict.judge([run()] * 5, [run()] * 5)
        self.assertEqual(result.regressions, [])
        self.assertTrue(all(row.status == 'ok' for row in result.rows))

    def test_a_clear_slowdown_of_one_endpoint_is_the_only_regression(self):
        result = verdict.judge([run(rps=1000)] * 5, [run(rps=600)] * 5)
        self.assertEqual([row.name for row in result.regressions], ['snapshot: req/s'])
        row = rows_by_name(result)['snapshot: req/s']
        self.assertEqual((row.base, row.head), (1000, 600))
        self.assertAlmostEqual(row.worse_by, 0.4)

    def test_a_change_inside_the_threshold_is_not_a_regression(self):
        result = verdict.judge([run(rps=1000)] * 5, [run(rps=900)] * 5)
        self.assertEqual(result.regressions, [])

    def test_a_slower_median_whose_runs_overlap_the_base_is_noise(self):
        base = [run(rps=r) for r in (60, 100, 100, 100, 100)]
        head = [run(rps=r) for r in (75, 75, 75, 75, 150)]
        result = verdict.judge(base, head)
        self.assertEqual(result.regressions, [])
        self.assertEqual(rows_by_name(result)['snapshot: req/s'].status, 'noisy')

    def test_a_faster_median_whose_runs_overlap_the_base_is_noise_too(self):
        base = [run(rps=r) for r in (100, 100, 100, 100, 100)]
        head = [run(rps=r) for r in (90, 150, 150, 150, 150)]
        self.assertEqual(rows_by_name(verdict.judge(base, head))['snapshot: req/s'].status, 'noisy')

    def test_for_times_and_sizes_higher_is_worse(self):
        slower = verdict.judge([run(ready_ms=100)] * 5, [run(ready_ms=150)] * 5)
        self.assertEqual([r.name for r in slower.regressions], ['start → first snapshot (ms)'])
        faster = verdict.judge([run(ready_ms=100)] * 5, [run(ready_ms=50)] * 5)
        self.assertEqual(faster.regressions, [])
        self.assertEqual(rows_by_name(faster)['start → first snapshot (ms)'].status, 'improved')

    def test_memory_cpu_and_the_edit_latency_are_gated_too(self):
        result = verdict.judge([run()] * 5, [run(rss_mb=90, cpu_s=9, push_ms=90)] * 5)
        self.assertEqual({r.name for r in result.regressions},
                         {'RSS after load (MB)', 'CPU for the whole run (s)', 'edit → pushed snapshot (ms)'})

    def test_a_change_smaller_than_the_measurement_floor_is_ignored(self):
        result = verdict.judge([run(ready_ms=2)] * 5, [run(ready_ms=4)] * 5)  # 100% of 2 ms
        self.assertEqual(result.regressions, [])

    def test_new_failed_requests_are_a_regression_whatever_the_speed(self):
        result = verdict.judge([run()] * 5, [run(non2xx=3)] * 5)
        self.assertEqual([r.name for r in result.regressions], ['snapshot: failed requests'])

    def test_failures_the_base_already_had_are_not_new(self):
        result = verdict.judge([run(failed=3)] * 5, [run(failed=3)] * 5)
        self.assertEqual(result.regressions, [])

    def test_a_run_that_never_produced_numbers_is_an_error_not_a_pass(self):
        broken = {'error': 'no url printed', 'stderr': 'boom'}
        with self.assertRaisesRegex(verdict.BenchFailed, 'no url printed'):
            verdict.judge([run()] * 3, [run(), broken, run()])

    def test_too_few_runs_to_tell_noise_from_change_is_an_error(self):
        with self.assertRaisesRegex(verdict.BenchFailed, 'at least'):
            verdict.judge([run()], [run()])


class Rendering(unittest.TestCase):
    def test_the_table_names_each_metric_with_both_medians_and_the_verdict(self):
        text = verdict.render(verdict.judge([run(rps=1000)] * 5, [run(rps=600)] * 5), threshold=0.2)
        lines = text.splitlines()
        self.assertIn('| metric | base | head | change | |', lines)
        row = next(line for line in lines if line.startswith('| snapshot: req/s'))
        self.assertEqual(row, '| snapshot: req/s | 1,000 | 600 | −40.0% | ❌ regression |')
        self.assertIn('1 regression', text)

    def test_a_clean_run_says_so(self):
        text = verdict.render(verdict.judge([run()] * 5, [run()] * 5), threshold=0.2)
        self.assertIn('No regression', text)
        self.assertIn('beyond 20%', text)


if __name__ == '__main__':
    unittest.main()
