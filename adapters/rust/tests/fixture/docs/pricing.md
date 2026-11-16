# Pricing

Prices are kept in cents. A discount is a whole percentage taken off a price.

```rust check
use shop::{apply_discount, Money};

let price = apply_discount(Money::eur(1000), 25).unwrap();
assert_eq!(price.to_string(), "7.50 EUR");
```

The same as a whole program:

```rust check
use shop::{total, Money};

fn main() {
    let total = total(&[Money::eur(1000)], "NL").unwrap();
    assert_eq!((total.cents, total.currency), (1210, "EUR"));
}
```

This block only has to compile against the library; it is never run.

```rust compile
use shop::{total, Money};

fn checkout(basket: &[Money]) -> Result<Money, String> {
    total(basket, "DE")
}
```

A block without an assertion runs, and counts as weak.

```rust check
use shop::Money;

println!("{}", Money::eur(5));
```

A line a reader need not see begins with `# `, as in a doc test.

```rust check
# use shop::Money;
assert_eq!(Money::eur(5).cents, 5);
```

Prices are in `EUR` unless told otherwise. <!-- fact: DEFAULT_CURRENCY -->
A discount is at most `50` percent. <!-- fact: MAX_DISCOUNT -->

Tax is known for these regions: <!-- fact: TAX_RATES -->
- `NL`
- `DE`
- `FR`

<!-- fact column=rate: rates(&["NL", "DE", "FR"]) -->
| region | rate |
|--------|------|
| NL     | 21%  |
| DE     | 19%  |
| FR     | 20%  |

The error messages, through a function of the setup module: <!-- fact: error_messages() -->
- The item is not available
- The discount is outside the allowed range
- No carrier serves the region

The regions the upstream API lists (a snapshot): <!-- fact ordered: snapshot("regions").iter().map(|region| region["code"].to_string()).collect::<Vec<_>>() -->
1. `NL`
2. `DE`
3. `FR`

The name of a currency is `3` letters long. <!-- fact: DEFAULT_CURRENCY.trim().len() -->

<!-- cases: discount -->
| case           | price | percent | result |
|----------------|-------|---------|--------|
| a quarter off  | 1000  | 25      | 750    |
| rounds half up | 999   | 50      | 500    |
| nothing off    | 1000  | 0       | 1000   |

Discounts above the maximum are refused. <!-- test: src/lib.rs > "tests" > "rejects_big_discounts" -->
