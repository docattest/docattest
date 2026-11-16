//! Snapshots: pinned facts about the outside world, loaded read-only (`src/snapshots/`).
//! `refresh` refetches them (`refresh.rs`).

pub mod refresh;

use serde_json::Value;

use crate::api::attestation::Kind;
use crate::files::project_files::ProjectFiles;
use crate::files::source::DiskFiles;
use crate::json::parse_json;
use crate::report::{Outcome, SnapshotStatus, Status};

const DAY_MS: f64 = 86_400_000.0;

/// A snapshot as committed.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub name: String,
    pub path: String,
    pub source: String,
    pub fetched_at: String,
    /// `data`, as the adapter receives it.
    pub data: Value,
}

/// Every snapshot in the snapshots directory, their ages, and the files that are broken.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LoadedSnapshots {
    pub by_name: Vec<Snapshot>,
    pub statuses: Vec<SnapshotStatus>,
    pub errors: Vec<Outcome>,
}

/// Load snapshots read-only; a broken file is an `error` result located at that file. `now`
/// is milliseconds since the epoch.
pub fn load_snapshots(files: &DiskFiles, dir: Option<&str>, now: f64, max_age_days: u64) -> LoadedSnapshots {
    let mut loaded = LoadedSnapshots::default();
    let Some(dir) = dir.filter(|d| !d.is_empty()) else {
        return loaded;
    };
    for file in files.list_directory(dir).into_iter().filter(|f| f.ends_with(".json")) {
        let base = &file[file.rfind('/').map_or(0, |k| k + 1)..];
        let name = base[..base.len() - ".json".len()].to_string();
        match read_snapshot(files, &file, &name) {
            Err(message) => {
                loaded.errors.push(Outcome::new(&file, 1, Kind::Snapshot, Status::Error, &message));
            }
            Ok((snapshot, fetched)) => {
                let age_days = ((now - fetched) / DAY_MS).floor();
                loaded.statuses.push(SnapshotStatus {
                    name: name.clone(),
                    source: snapshot.source.clone(),
                    fetched_at: snapshot.fetched_at.clone(),
                    age_days,
                    warning: age_days > max_age_days as f64,
                });
                loaded.by_name.push(snapshot);
            }
        }
    }
    loaded
}

/// The snapshot in one file and the instant it was fetched, or what is wrong with it.
fn read_snapshot(files: &DiskFiles, file: &str, name: &str) -> Result<(Snapshot, f64), String> {
    let text = files.read(file)?;
    let parsed = parse_json(&text).map_err(|m| format!("snapshot {name} is not valid JSON: {m}"))?;
    let shape =
        || format!("snapshot {name} needs \"source\" (string), \"fetchedAt\" (YYYY-MM-DD) and \"data\"");
    let Value::Object(object) = &parsed else {
        return Err(shape());
    };
    let (Some(Value::String(source)), Some(Value::String(fetched_at)), Some(data)) =
        (object.get("source"), object.get("fetchedAt"), object.get("data"))
    else {
        return Err(shape());
    };
    let Some(fetched) = instant(fetched_at) else {
        return Err(format!("snapshot {name}: fetchedAt \"{fetched_at}\" is not a date"));
    };
    let snapshot = Snapshot {
        name: name.to_string(),
        path: file.to_string(),
        source: source.clone(),
        fetched_at: fetched_at.clone(),
        data: data.clone(),
    };
    Ok((snapshot, fetched))
}

/// The time now in milliseconds since the epoch, unless `DOCATTEST_NOW` names another instant
/// (as [`instant`] reads it): the clock a snapshot's age and `refresh`'s `fetchedAt` are
/// measured by. The programmatic API sets it for `env.now`, and the parity harnesses replay
/// their recordings at the instant they were recorded. A value that is not a time is ignored.
pub fn now_ms() -> f64 {
    if let Some(now) = std::env::var("DOCATTEST_NOW").ok().as_deref().and_then(instant) {
        return now;
    }
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_millis() as f64)
}

/// Milliseconds since the epoch at `text`: a date (`YYYY-MM-DD`, midnight UTC), which is what
/// `refresh` writes, or an RFC 3339 time.
fn instant(text: &str) -> Option<f64> {
    if let Ok(date) = chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return Some(date.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis() as f64);
    }
    chrono::DateTime::parse_from_rfc3339(text).ok().map(|time| time.timestamp_millis() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_instant_is_a_date_or_an_rfc_3339_time() {
        assert_eq!(instant("2023-11-14"), Some(1_699_920_000_000.0));
        assert_eq!(instant("2023-11-14T22:13:20.000Z"), Some(1_700_000_000_000.0));
        assert_eq!(instant("2023-11-14T23:13:20+01:00"), Some(1_700_000_000_000.0));
        assert_eq!(instant("14 November 2023"), None);
        assert_eq!(instant("nope"), None);
    }
}
