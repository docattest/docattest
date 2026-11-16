"""Carriers and shipping quotes."""

from enum import Enum

from .errors import ShopError


class Carrier(Enum):
    POST = "post"
    COURIER = "courier"
    DRONE = "drone"


CARRIERS = [c.value for c in Carrier]

ROUTES = [
    {"region": "NL", "carrier": "post", "days": 1},
    {"region": "DE", "carrier": "post", "days": 2},
    {"region": "FR", "carrier": "courier", "days": 3},
]


def quote(region: str) -> int:
    """Delivery time in days for a region."""
    for route in ROUTES:
        if route["region"] == region:
            return route["days"]
    raise ShopError("unknown_region")


async def fetch_rates(region: str) -> dict:
    """Live rates for a region (pretends to ask a carrier)."""
    return {"region": region, "days": quote(region)}
