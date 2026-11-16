//! Who delivers, and how fast.

/// The carriers that deliver.
pub const CARRIERS: [&str; 3] = ["post", "courier", "drone"];

/// The delivery times in days, by region.
const DAYS: [(&str, u32); 2] = [("NL", 1), ("DE", 2)];

/// The number of days a delivery to a region takes.
pub fn quote(region: &str) -> Result<u32, String> {
    DAYS.iter()
        .find(|(code, _)| *code == region)
        .map(|(_, days)| *days)
        .ok_or(format!("no carrier serves {region:?}"))
}
