# Pricing

Prices are kept in cents. A discount is a whole percentage taken off a price.

```go check
import (
	"fmt"

	"example.com/shop"
)

price, err := shop.ApplyDiscount(shop.EUR(1000), 25)
if err != nil {
	panic(err)
}
fmt.Println(price)
// Output: 7.50 EUR
```

The same as a whole program:

```go check
package main

import (
	"fmt"

	"example.com/shop"
)

func main() {
	total, _ := shop.Total([]shop.Money{shop.EUR(1000)}, "NL")
	fmt.Println(total.Cents, total.Currency)
	// Output: 1210 EUR
}
```

This block only has to compile against the library; it is never run.

```go compile
import "example.com/shop"

func checkout(basket []shop.Money) (shop.Money, error) {
	return shop.Total(basket, "DE")
}

var _ = checkout
```

A block without an assertion runs, and counts as weak.

```go check
import (
	"fmt"

	"example.com/shop"
)

fmt.Println(shop.EUR(5))
```

Prices are in `EUR` unless told otherwise. <!-- fact: DefaultCurrency -->
A discount is at most `50` percent. <!-- fact: MaxDiscount -->

Tax is known for these regions: <!-- fact: TaxRates -->
- `NL`
- `DE`
- `FR`

<!-- fact column=rate: Rates("NL", "DE", "FR") -->
| region | rate |
|--------|------|
| NL     | 21%  |
| DE     | 19%  |
| FR     | 20%  |

The error messages, through a function of the setup package: <!-- fact: ErrorMessages() -->
- The item is not available
- The discount is outside the allowed range
- No carrier serves the region

The regions the upstream API lists (a snapshot): <!-- fact ordered: Codes(snapshot("regions")) -->
1. `NL`
2. `DE`
3. `FR`

The name of a currency is `3` letters long. <!-- fact: len(strings.TrimSpace(DefaultCurrency)) -->

<!-- cases: discount -->
| case           | price | percent | result |
|----------------|-------|---------|--------|
| a quarter off  | 1000  | 25      | 750    |
| rounds half up | 999   | 50      | 500    |
| nothing off    | 1000  | 0       | 1000   |

Discounts above the maximum are refused. <!-- test: shop_test.go > "TestTotals" > "rejects big discounts" -->
