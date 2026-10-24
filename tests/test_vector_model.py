import math
import unittest
from research.vector_model import decide

class DynamicVectorTests(unittest.TestCase):
    def test_attention_rows_are_normalized(self):
        result = decide(.4, .001, 3, .001, 4, 0)
        for row in result["attention"]:
            self.assertAlmostEqual(sum(row), 1.0, places=9)
            self.assertTrue(all(math.isfinite(weight) and weight >= 0 for weight in row))
    def test_missing_spread_abstains(self):
        self.assertEqual(decide(.9, None, None, .001, 4)["action"], "wait")
    def test_attention_changes_with_market_state(self):
        a = decide(.8, .001, 3, .001, 4, 0)
        b = decide(-.8, -.001, 3, .001, 4, 0)
        self.assertNotEqual(a["attention"], b["attention"])
    def test_non_finite_feature_rejected(self):
        with self.assertRaises(ValueError):
            decide(float("nan"), None, 3, .001, 4)
if __name__ == "__main__":
    unittest.main()
