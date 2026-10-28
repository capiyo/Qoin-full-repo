import unittest
from research.reference_model import signed_flow, queue_imbalance, decay_intensity

class FlowModelTests(unittest.TestCase):
    def test_signed_flow(self):
        self.assertEqual(signed_flow([("buy", 4), ("sell", 1), ("unknown", 99)]), 3)
    def test_queue_imbalance(self):
        self.assertAlmostEqual(queue_imbalance(3, 1), 0.5)
        self.assertEqual(queue_imbalance(0, 0), 0)
    def test_intensity_decays(self):
        self.assertLess(decay_intensity(2, 30), 2)
        self.assertAlmostEqual(decay_intensity(2, 0), 2)
    def test_rejects_invalid_size(self):
        with self.assertRaises(ValueError):
            signed_flow([("buy", float("nan"))])
if __name__ == "__main__":
    unittest.main()
