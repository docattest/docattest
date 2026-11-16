//! The setup of the pages in this directory: the runners of their cases tables, and
//! what their fact expressions use by name. It is no part of the `shop` crate: docattest
//! compiles it, in a program of its own that depends on the crate.

use shop::{apply_discount, shipping, Money, ERROR_CODES, TAX_RATES};
use std::collections::HashMap;

type Row = HashMap<String, String>;

/// The runners of cases tables, by name.
pub const CASES: &[(&str, fn(&Row) -> Result<(), String>)] =
    &[("discount", discount), ("delivery", delivery)];

/// The columns each runner's table must have.
pub const COLUMNS: &[(&str, &[&str])] = &[("discount", &["price", "percent", "result"])];

fn discount(row: &Row) -> Result<(), String> {
    let number = |column: &str| row[column].parse::<i64>().unwrap_or(0);
    let got = apply_discount(Money::eur(number("price")), number("percent") as u32)?;
    if got.cents.to_string() == row["result"] {
        Ok(())
    } else {
        Err(format!("expected {}, got {}", row["result"], got.cents))
    }
}

fn delivery(row: &Row) -> Result<(), String> {
    let got = shipping::quote(&row["region"])?;
    if got.to_string() == row["days"] {
        Ok(())
    } else {
        Err(format!("expected {} days, got {got}", row["days"]))
    }
}

/// The messages of the shop's errors.
pub fn error_messages() -> Vec<&'static str> {
    ERROR_CODES.iter().map(|(_, message)| *message).collect()
}

/// The tax rates as a page writes them, for the regions given.
pub fn rates(regions: &[&str]) -> Vec<String> {
    let rate = |region: &&str| {
        TAX_RATES
            .iter()
            .find(|(code, _)| code == region)
            .map_or(0, |(_, rate)| *rate)
    };
    regions
        .iter()
        .map(|region| format!("{}%", rate(region)))
        .collect()
}
