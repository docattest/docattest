# Shipping

These carriers deliver: <!-- fact @shipping: Carriers -->
- `post`
- `courier`
- `drone`

A quote says how many days a delivery takes.

```go check
import "example.com/shop/shipping"

days, err := shipping.Quote("DE")
if err != nil || days != 2 {
	panic("a delivery to DE takes 2 days")
}
```

Live rates need the network, which is off unless allowed.

```go check network
import "net/http"

response, err := http.Get("https://example.com")
if err != nil || response.StatusCode != 200 {
	panic("example.com did not answer")
}
```

<!-- cases: delivery -->
| case        | region | days |
|-------------|--------|------|
| home        | NL     | 1    |
| next door   | DE     | 2    |
