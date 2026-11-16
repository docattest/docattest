# Pricing

Prices are kept in cents. A discount is a whole percentage taken off a price.

```python check
from shoplib import Money, apply_discount

assert apply_discount(Money(1000), 25) == Money(750)
assert str(apply_discount(Money(999), 50)) == "5.00 EUR"
```

The same in an interpreter session:

```pycon check
>>> from shoplib import Money, total
>>> total([Money(1000)], "NL")
Money(cents=1210, currency='EUR')
```

This block only has to be valid against the library; it is never run.

```python compile
from shoplib import Money, total

def checkout(basket: list[Money]) -> Money:
    return total(basket, region="DE")
```

A block without an assertion runs, and counts as weak.

```python check
from shoplib import Money
print(Money(5))
```

Prices are in `EUR` unless told otherwise. <!-- fact: DEFAULT_CURRENCY -->
A discount is at most 50 percent. <!-- fact: MAX_DISCOUNT -->

Tax is known for these regions: <!-- fact: TAX_RATES -->
- `NL`
- `DE`
- `FR`

<!-- fact column=rate: [f"{r}%" for r in TAX_RATES.values()] -->
| region | rate |
|--------|------|
| NL     | 21%  |
| DE     | 19%  |
| FR     | 20%  |

The error messages, through a named fact of the setup module: <!-- fact: error_messages() -->
- The item is not available
- The discount is outside the allowed range
- No carrier serves the region

The regions the upstream API lists (a snapshot): <!-- fact ordered: [r["code"] for r in snapshot("regions")] -->
1. `NL`
2. `DE`
3. `FR`

<!-- cases: discount -->
| case           | price | percent | result |
|----------------|-------|---------|--------|
| a quarter off  | 1000  | 25      | 750    |
| rounds half up | 999   | 50      | 500    |
| nothing off    | 1000  | 0       | 1000   |

Discounts above the maximum are refused. <!-- test: tests/test_pricing.py > "TestTotals" > "test_rejects_big_discounts" -->
