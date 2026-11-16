//! Prices baskets: money, discounts and tax.

pub mod shipping;

use std::fmt;

/// The currency of a price that names none.
pub const DEFAULT_CURRENCY: &str = "EUR";

/// The largest discount, in percent.
pub const MAX_DISCOUNT: u32 = 50;

/// The tax rates in percent, by region.
pub const TAX_RATES: [(&str, u32); 3] = [("NL", 21), ("DE", 19), ("FR", 20)];

/// The messages of the errors this crate returns, by code.
pub const ERROR_CODES: [(&str, &str); 3] = [
    ("out_of_stock", "The item is not available"),
    ("bad_discount", "The discount is outside the allowed range"),
    ("no_carrier", "No carrier serves the region"),
];

/// An amount in the cents of a currency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Money {
    pub cents: i64,
    pub currency: &'static str,
}

impl Money {
    /// An amount of cents in the default currency.
    pub fn eur(cents: i64) -> Money {
        Money { cents, currency: DEFAULT_CURRENCY }
    }
}

/// An amount as a reader expects it: `7.50 EUR`.
impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02} {}", self.cents / 100, self.cents % 100, self.currency)
    }
}

/// Takes a percentage off a price, rounding half up to a whole cent.
///
/// ```
/// use shop::{apply_discount, Money};
///
/// assert_eq!(apply_discount(Money::eur(999), 50), Ok(Money::eur(500)));
/// ```
pub fn apply_discount(price: Money, percent: u32) -> Result<Money, String> {
    if percent > MAX_DISCOUNT {
        return Err(ERROR_CODES[1].1.to_string());
    }
    Ok(Money { cents: round(price.cents * (100 - i64::from(percent))), ..price })
}

/// Adds up a basket and the tax of a region.
///
/// ```no_run
/// let total = shop::total(&[shop::Money::eur(1000)], "NL");
/// ```
pub fn total(basket: &[Money], region: &str) -> Result<Money, String> {
    let (_, rate) = TAX_RATES
        .iter()
        .find(|(code, _)| *code == region)
        .ok_or(format!("no tax rate for region {region:?}"))?;
    let sum: i64 = basket.iter().map(|price| price.cents).sum();
    Ok(Money::eur(round(sum * (100 + i64::from(*rate)))))
}

/// Divides hundredths of a cent by a hundred, half up.
fn round(hundredths: i64) -> i64 {
    (hundredths + 50) / 100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_tax() {
        assert_eq!(total(&[Money::eur(1000)], "NL"), Ok(Money::eur(1210)));
    }

    #[test]
    fn rejects_big_discounts() {
        assert!(apply_discount(Money::eur(1000), 90).is_err());
    }
}
