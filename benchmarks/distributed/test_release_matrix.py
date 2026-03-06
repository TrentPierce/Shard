import unittest

from benchmarks.distributed.run_release_matrix import Scenario, scenario_inference_mode


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


if __name__ == "__main__":
    unittest.main()
