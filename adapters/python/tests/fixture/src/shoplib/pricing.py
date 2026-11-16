"""Prices, discounts and tax."""

from dataclasses import dataclass

from .errors import ShopError

DEFAULT_CURRENCY = "EUR"
MAX_DISCOUNT = 50
TAX_RATES = {"NL": 21, "DE": 19, "FR": 20}

RECEIPT_TEMPLATE = """\
{item}  {price}
tax     {tax}
total   {total}
"""


@dataclass(frozen=True)
class Money:
    """An amount in cents with its currency.

    >>> Money(1250)
    Money(cents=1250, currency='EUR')
    >>> str(Money(1250))
    '12.50 EUR'
    """

    cents: int
    currency: str = DEFAULT_CURRENCY

    def __str__(self) -> str:
        return f"{self.cents / 100:.2f} {self.currency}"


def _round(value: float) -> int:
    return int(value + 0.5)


def apply_discount(price: Money, percent: int) -> Money:
    """Take a percentage off a price, rounding half up to a whole cent.

    >>> apply_discount(Money(1000), 25)
    Money(cents=750, currency='EUR')
    >>> apply_discount(Money(1000), 80)
    Traceback (most recent call last):
        ...
    shoplib.errors.ShopError: The discount is outside the allowed range
    """
    if not 0 <= percent <= MAX_DISCOUNT:
        raise ShopError("bad_discount")
    return Money(_round(price.cents * (100 - percent) / 100), price.currency)


def total(prices: list, region: str = "NL") -> Money:
    """The sum of some prices with the region's tax added.

    Args:
        prices: what is in the basket.
        region: whose tax rate applies.

    >>> total([Money(1000), Money(500)], "DE").cents
    1785
    """
    net = sum(p.cents for p in prices)
    return Money(_round(net * (100 + TAX_RATES[region]) / 100))
