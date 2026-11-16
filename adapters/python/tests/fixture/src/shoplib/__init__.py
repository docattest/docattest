"""A small shop library: the fixture the Python adapter's evidence runs against."""

from .errors import ERROR_CODES, ShopError
from .pricing import DEFAULT_CURRENCY, MAX_DISCOUNT, TAX_RATES, Money, apply_discount, total
from .shipping import CARRIERS, ROUTES, Carrier, fetch_rates, quote

__all__ = [
    "CARRIERS",
    "Carrier",
    "DEFAULT_CURRENCY",
    "ERROR_CODES",
    "MAX_DISCOUNT",
    "Money",
    "ROUTES",
    "ShopError",
    "TAX_RATES",
    "apply_discount",
    "fetch_rates",
    "quote",
    "total",
]
