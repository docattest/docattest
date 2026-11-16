# Shipping

These carriers deliver: <!-- fact @shipping: CARRIERS -->
- `post`
- `courier`
- `drone`

A quote says how many days a delivery takes.

```rust check
use shop::shipping;

assert_eq!(shipping::quote("DE"), Ok(2));
```

A region nobody serves is refused, and unwrapping the refusal panics.

```rust check should_panic
shop::shipping::quote("XX").unwrap();
```

Live rates need the network, which is off unless allowed.

```rust check network
use std::net::TcpStream;

assert!(TcpStream::connect("example.com:80").is_ok());
```

<!-- cases: delivery -->
| case        | region | days |
|-------------|--------|------|
| home        | NL     | 1    |
| next door   | DE     | 2    |
