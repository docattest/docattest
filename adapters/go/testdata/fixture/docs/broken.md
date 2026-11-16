# Deliberately wrong

Every attestation on this page fails or is an error, each in its own way.

```go check
import (
	"fmt"

	"example.com/shop"
)

price, _ := shop.ApplyDiscount(shop.EUR(1000), 25)
fmt.Println(price)
// Output: 7.00 EUR
```

```go compile
import "example.com/shop"

var _ = shop.AplyDiscount
```

```go check
import "example.com/shop/shipping"

if _, err := shipping.Quote("XX"); err != nil {
	panic(err)
}
```

```go check
for {
}
```

```go check
import "net/http"

if _, err := http.Get("https://example.com"); err != nil {
	panic(err)
}
```

```go chekc
panic("never read")
```

A discount is at most `60` percent. <!-- fact: MaxDiscount -->

Tax is known for these regions: <!-- fact: TaxRates -->
- `NL`
- `BE`

The private rounding helper gives `1`. <!-- fact: round(50) -->

<!-- cases: discount -->
| case             | price | percent | result |
|------------------|-------|---------|--------|
| wrong on purpose | 1000  | 25      | 800    |
| too much         | 1000  | 90      | 100    |

<!-- cases: nope -->
| case | result |
|------|--------|
| a    | 1      |

It is tested. <!-- test: shop_test.go > "TestThatWasRenamed" -->

It is measured. <!-- test: shop_test.go > "BenchmarkTotal" -->

<!-- cases: total -->
| case                    | basket   | region | total |
|-------------------------|----------|--------|-------|
| a price in words        | 1000 ten | NL     | 1210  |
| the next row still runs | 1000     | NL     | 1200  |
