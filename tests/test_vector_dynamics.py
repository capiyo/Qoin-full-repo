import unittest
from research.vector_dynamics import (
    VectorObservation, derive_kinematics, pairwise_interactions, pareto_top_k,
)

class VectorDynamicsTests(unittest.TestCase):
    def test_causal_position_velocity_acceleration(self):
        old = VectorObservation(1000, {"ofi": 0.0})
        previous = VectorObservation(2000, {"ofi": 1.0})
        current = VectorObservation(3000, {"ofi": 3.0})
        self.assertEqual(derive_kinematics(old, previous, current)["ofi"],
                         {"position": 3.0, "velocity": 2.0, "acceleration": 1.0})

    def test_rejects_out_of_order_observations(self):
        with self.assertRaises(ValueError):
            derive_kinematics(VectorObservation(2000, {"x": 1}),
                              VectorObservation(1000, {"x": 2}),
                              VectorObservation(3000, {"x": 3}))

    def test_regularized_pairwise_interaction(self):
        result = pairwise_interactions({"a": 1.0, "b": 1.0}, regularization=1.0)
        self.assertAlmostEqual(result["a*b"], 0.5)

    def test_pareto_frontier_prioritizes_reliable_forces(self):
        result = pareto_top_k({"a": 0.9, "b": 0.7, "c": 0.2},
                              {"a": 0.4, "b": 0.9, "c": 0.9}, 2)
        self.assertEqual(result, ["b", "a"])

if __name__ == "__main__":
    unittest.main()
