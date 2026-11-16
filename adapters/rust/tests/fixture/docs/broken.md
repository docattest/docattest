# Deliberately wrong

Every attestation on this page fails or is an error, each in its own way.

```rust check
use shop::{apply_discount, Money};

assert_eq!(apply_discount(Money::eur(1000), 25), Ok(Money::eur(700)));
```

```rust compile
use shop::aply_discount;
```

```rust check
shop::shipping::quote("XX").unwrap();
```

```rust check
loop {}
```

```rust check should_panic
assert_eq!(shop::MAX_DISCOUNT, 50);
```

```rust chekc
panic!("never read");
```

A discount is at most `60` percent. <!-- fact: MAX_DISCOUNT -->

Tax is known for these regions: <!-- fact: TAX_RATES -->
- `NL`
- `BE`

The private rounding helper gives `1`. <!-- fact: round(50) -->

<!-- cases: discount -->
| case             | price               | percent | result |
|------------------|---------------------|---------|--------|
| wrong on purpose | 1000                | 25      | 800    |
| overflows        | 9223372036854775807 | 25      | 0      |
| too much         | 1000                | 90      | 100    |

<!-- cases: nope -->
| case | result |
|------|--------|
| a    | 1      |

It is tested. <!-- test: src/lib.rs > "test_that_was_renamed" -->

It is tested too. <!-- test: src/lib.rs > "tests" -->
