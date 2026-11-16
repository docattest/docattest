# Shipping

These carriers deliver: <!-- fact @shipping: CARRIERS -->
- `post`
- `courier`
- `drone`

A quote can be awaited from the carrier.

```python check
from shoplib import fetch_rates

rates = await fetch_rates("DE")
assert rates == {"region": "DE", "days": 2}
```

Live rates need the network, which is off unless allowed.

```python check network
import urllib.request
assert urllib.request.urlopen("https://example.com").status == 200
```

<!-- cases: delivery -->
| case        | region | days |
|-------------|--------|------|
| home        | NL     | 1    |
| next door   | DE     | 2    |
