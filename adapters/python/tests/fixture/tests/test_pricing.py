import pytest

from shoplib import Money, ShopError, apply_discount, total


def test_discount_rounds_half_up():
    assert apply_discount(Money(999), 50).cents == 500


class TestTotals:
    def test_adds_tax(self):
        assert total([Money(1000)], "NL").cents == 1210

    def test_rejects_big_discounts(self):
        with pytest.raises(ShopError):
            apply_discount(Money(1000), 90)
