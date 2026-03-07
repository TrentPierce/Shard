import unittest

from benchmarks.distributed.run_release_matrix import (
    Scenario,
    evaluate_release_gates,
    resolve_matrix_class,
    scenario_inference_mode,
)


class ReleaseMatrixScenarioTests(unittest.TestCase):
    def test_no_scout_scenarios_force_standard_mode(self) -> None:
        scenario = Scenario(
            name="one-node-no-scouts",
            use_scout_workers=False,
            pool_kind="one_node",
        )
        self.assertEqual(scenario_inference_mode(scenario, "distributed"), "standard")

    def test_with_scout_scenarios_keep_requested_mode(self) -> None:
        scenario = Scenario(
            name="two-node-with-scouts",
            use_scout_workers=True,
            pool_kind="two_node",
        )
        self.assertEqual(scenario_inference_mode(scenario, "distributed"), "distributed")

    def test_short_rc_stability_gates_do_not_require_scout_uplift(self) -> None:
        gates = evaluate_release_gates(
            {
                "two-node-no-scouts": {
                    "p95_latency_ms_median": 526.015,
                    "throughput_tps_median": 1.9,
                    "all_orchestrator_runs_passed": True,
                },
                "two-node-with-scouts": {
                    "p95_latency_ms_median": 662.13,
                    "error_rate_pct_median": 0.0,
                    "timeout_rate_pct_median": 0.0,
                    "http_429_rate_pct_median": 0.0,
                    "http_503_rate_pct_median": 0.0,
                    "throughput_tps_median": 1.7,
                    "speculative_samples_median": 0.0,
                    "all_orchestrator_runs_passed": True,
                },
            },
            resolve_matrix_class("short_rc_stability"),
        )
        self.assertTrue(all(bool(gate["pass"]) for gate in gates))

    def test_long_scout_generation_requires_speculative_samples(self) -> None:
        gates = evaluate_release_gates(
            {
                "two-node-no-scouts": {
                    "p95_latency_ms_median": 526.015,
                    "throughput_tps_median": 1.9,
                    "all_orchestrator_runs_passed": True,
                },
                "two-node-with-scouts": {
                    "p95_latency_ms_median": 662.13,
                    "error_rate_pct_median": 0.0,
                    "timeout_rate_pct_median": 0.0,
                    "http_429_rate_pct_median": 0.0,
                    "http_503_rate_pct_median": 0.0,
                    "throughput_tps_median": 1.7,
                    "speculative_samples_median": 0.0,
                    "all_orchestrator_runs_passed": True,
                },
            },
            resolve_matrix_class("long_scout_generation"),
        )
        speculative_gate = next(
            gate for gate in gates if gate["name"] == "two_node_with_scouts_speculative_samples"
        )
        self.assertFalse(bool(speculative_gate["pass"]))

    def test_long_scout_generation_uses_relative_uplift_not_short_absolute_tps_gate(self) -> None:
        gates = evaluate_release_gates(
            {
                "two-node-no-scouts": {
                    "p95_latency_ms_median": 1500.0,
                    "throughput_tps_median": 1.2,
                    "all_orchestrator_runs_passed": True,
                },
                "two-node-with-scouts": {
                    "p95_latency_ms_median": 1550.0,
                    "error_rate_pct_median": 0.0,
                    "timeout_rate_pct_median": 0.0,
                    "http_429_rate_pct_median": 0.0,
                    "http_503_rate_pct_median": 0.0,
                    "throughput_tps_median": 1.3,
                    "speculative_samples_median": 10.0,
                    "all_orchestrator_runs_passed": True,
                },
            },
            resolve_matrix_class("long_scout_generation"),
        )
        self.assertFalse(any(gate["name"] == "two_node_with_scouts_tps" for gate in gates))
        uplift_gate = next(
            gate for gate in gates if gate["name"] == "two_node_with_scouts_tps_vs_no_scouts"
        )
        self.assertTrue(bool(uplift_gate["pass"]))


if __name__ == "__main__":
    unittest.main()
