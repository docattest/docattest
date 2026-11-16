# Deliberately wrong

Every attestation on this page fails or is an error, each in its own way.

```python check
from shoplib import Money, apply_discount

assert apply_discount(Money(1000), 25) == Money(700)
```

```pycon check
>>> from shoplib import quote
>>> quote("NL")
2
```

```python compile
from shoplib import Money, aply_discount
```

```python check
from shoplib import quote
quote("XX")
```

```python check
while True:
    pass
```

```python check
import urllib.request
urllib.request.urlopen("https://example.com")
```

```python chekc
assert True
```

A discount is at most 60 percent. <!-- fact: MAX_DISCOUNT -->

Tax is known for these regions: <!-- fact: TAX_RATES -->
- `NL`
- `BE`

The private rounding helper is `_round`. <!-- fact: _round(0.5) -->

<!-- cases: discount -->
| case            | price | percent | result |
|-----------------|-------|---------|--------|
| wrong on purpose | 1000 | 25      | 800    |
| too much        | 1000  | 90      | 100    |

<!-- cases: nope -->
| case | result |
|------|--------|
| a    | 1      |

It is tested. <!-- test: tests/test_pricing.py > "test_that_was_renamed" -->
