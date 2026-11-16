"""Runners for cases tables and named facts, for the pages in this directory."""

from shoplib import ERROR_CODES, Money, apply_discount, quote


def discount(row):
    got = apply_discount(Money(int(row["price"])), int(row["percent"]))
    assert str(got.cents) == row["result"]


discount.columns = ["price", "percent", "result"]


def delivery(row):
    assert str(quote(row["region"])) == row["days"]


cases = {"discount": discount, "delivery": delivery}

facts = {"error_messages": lambda: list(ERROR_CODES.values())}
