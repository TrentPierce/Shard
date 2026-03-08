import unittest

from benchmarks.distributed.run_release_matrix import (
    Scenario,
    evaluate_release_gates,
    resolve_matrix_class,
    scenario_inference_mode,
    validate_long_profile_observation,
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

    def test_long_scout_generation_uses_auto_thresholded_browser_warmup_tokens(self) -> None:
        matrix = resolve_matrix_class("long_scout_generation")
        self.assertIsNone(matrix.default_browser_warmup_request_max_tokens)

    def test_validate_long_profile_observation_accepts_matching_snapshot(self) -> None:
        failures = validate_long_profile_observation(
            endpoint="http://127.0.0.1:9191",
            health={"verifier_queue_cap": 6, "ready_for_inference": True},
            scout_config={
                "config": {
                    "profile": "long-benchmark-2026-03-06",
                    "speculative": {
                        "long_request_min_tokens": 32,
                        "timeout": {"verifier_ratio_long": 1.5},
                    },
                }
            },
            expected={
                "release_profile": "long-benchmark-2026-03-06",
                "verifier_queue_cap": 6,
                "long_request_min_tokens": 32,
                "verifier_ratio_long": 1.5,
            },
        )
        self.assertEqual(failures, [])

    def test_validate_long_profile_observation_reports_profile_mismatches(self) -> None:
        failures = validate_long_profile_observation(
            endpoint="http://35.175.242.222:9091",
            health={"verifier_queue_cap": 2, "ready_for_inference": False},
            scout_config={
                "config": {
                    "profile": "rc1-2026-03-04",
                    "speculative": {
                        "long_request_min_tokens": 0,
                        "timeout": {},
                    },
                }
            },
            expected={
                "release_profile": "long-benchmark-2026-03-06",
                "verifier_queue_cap": 6,
                "long_request_min_tokens": 32,
                "verifier_ratio_long": 1.5,
            },
        )
        self.assertTrue(any("profile=rc1-2026-03-04 expected=long-benchmark-2026-03-06" in item for item in failures))
        self.assertTrue(any("verifier_queue_cap=2 expected=6" in item for item in failures))
        self.assertTrue(any("long_request_min_tokens=0 expected=32" in item for item in failures))
        self.assertTrue(any("timeout.verifier_ratio_long missing" in item for item in failures))
        self.assertTrue(any("ready_for_inference=false" in item for item in failures))


if __name__ == "__main__":
    unittest.main()
