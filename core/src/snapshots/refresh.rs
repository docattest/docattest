//! `refresh`: refetch snapshots from their source and rewrite their files.
//!
//! The only part of the core that touches the network. The client is built when a snapshot
//! is fetched, so no other command pays for it.

use crate::api::errors::{Error, Result};
use crate::files::text_file;
use crate::json::{parse_json, pretty};
use crate::project::state::State;
use crate::snapshots::Snapshot;
use serde_json::{Map, Value};

/// What a fetch answered: the status and the body's bytes.
pub struct Fetched {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Why a fetch gave no answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// The source is not an http or https URL.
    BadUrl,
    /// No answer: no connection, no such host, too many redirects, …
    Failed,
}

/// A GET of a snapshot's source: it follows up to 20 redirects, answers any status, and takes
/// no proxy from the environment.
pub fn fetch(url: &str) -> std::result::Result<Fetched, FetchError> {
    let http = url.get(..5).is_some_and(|s| s.eq_ignore_ascii_case("http:"))
        || url.get(..6).is_some_and(|s| s.eq_ignore_ascii_case("https:"));
    if !http {
        return Err(FetchError::BadUrl);
    }
    let agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(20)
        .proxy(None)
        .user_agent("docattest")
        .accept("*/*")
        .build()
        .new_agent();
    let mut response = agent.get(url).call().map_err(|e| match e {
        ureq::Error::BadUri(_) | ureq::Error::Http(_) => FetchError::BadUrl,
        _ => FetchError::Failed,
    })?;
    let status = response.status().as_u16();
    let body =
        response.body_mut().with_config().limit(u64::MAX).read_to_vec().map_err(|_| FetchError::Failed)?;
    Ok(Fetched { status, body })
}

/// The UTC date of `now_ms`, as `YYYY-MM-DD`.
pub fn iso_date(now_ms: f64) -> String {
    chrono::DateTime::from_timestamp_millis(now_ms as i64)
        .map(|date| date.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

/// The new content of a snapshot's file from what its source answered.
pub fn snapshot_file(
    snapshot: &Snapshot,
    fetched: std::result::Result<Fetched, FetchError>,
    now_ms: f64,
) -> Result<String> {
    let (name, source) = (&snapshot.name, &snapshot.source);
    let fetched = fetched.map_err(|e| {
        Error::user(match e {
            FetchError::BadUrl => format!("refresh {name}: {source} is not an http or https URL"),
            FetchError::Failed => format!("refresh {name}: {source} did not answer"),
        })
    })?;
    if !(200..300).contains(&fetched.status) {
        return Err(Error::user(format!(
            "refresh {}: {} answered {}",
            snapshot.name, snapshot.source, fetched.status
        )));
    }
    // `response.json()`: decoded as UTF-8 with replacement, a byte order mark dropped.
    let text = String::from_utf8_lossy(&fetched.body);
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
    let data = parse_json(text)
        .map_err(|m| Error::user(format!("refresh {name}: {source} did not answer JSON ({m})")))?;
    let mut file = Map::new();
    file.insert("source".into(), Value::String(snapshot.source.clone()));
    file.insert("fetchedAt".into(), Value::String(iso_date(now_ms)));
    file.insert("data".into(), data);
    Ok(format!("{}\n", pretty(&Value::Object(file))))
}

/// Refetch one snapshot, or all of them, and rewrite their files. Returns the files
/// written, in order. A failure stops there: the files before it stay rewritten.
pub fn refresh_snapshots(state: &State, name: Option<&str>, now_ms: f64) -> Result<Vec<String>> {
    let all = &state.snapshots.by_name;
    let targets: Vec<&Snapshot> = match name {
        None => all.iter().collect(),
        Some(name) => match all.iter().find(|s| s.name == name) {
            Some(target) => vec![target],
            None => {
                let dir = state.config.snapshots_dir.as_deref().unwrap_or("(no snapshots dir configured)");
                return Err(Error::user(format!("no snapshot named \"{name}\" in {dir}")));
            }
        },
    };
    let mut written = Vec::new();
    for snapshot in targets {
        let content = snapshot_file(snapshot, fetch(&snapshot.source), now_ms)?;
        text_file::write_text(&state.files.path(&snapshot.path), &content).map_err(Error::user)?;
        if !written.contains(&snapshot.path) {
            written.push(snapshot.path.clone());
        }
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    /// A server on a local port that answers every request with `status` and `body`, and
    /// redirects `/moved` to `/data`. Returns its origin and the request lines it saw.
    fn serve(
        status: u16,
        body: &'static [u8],
        requests: usize,
    ) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for stream in listener.incoming().take(requests) {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut head = Vec::new();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                    head.push(line.trim_end().to_lowercase());
                }
                let moved = head.first().is_some_and(|l| l.starts_with("get /moved "));
                seen.extend(head);
                let answer = if moved {
                    b"HTTP/1.1 302 Found\r\nLocation: /data\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .to_vec()
                } else {
                    let mut answer = format!(
                        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .into_bytes();
                    answer.extend_from_slice(body);
                    answer
                };
                stream.write_all(&answer).unwrap();
            }
            seen
        });
        (origin, handle)
    }

    fn snapshot(source: &str) -> Snapshot {
        Snapshot {
            name: "regions".into(),
            path: "docs/.snapshots/regions.json".into(),
            source: source.into(),
            fetched_at: "2020-01-01".into(),
            data: serde_json::Value::Null,
        }
    }

    const NOW: f64 = 1_700_000_000_000.0; // 2023-11-14T22:13:20Z

    #[test]
    fn rewrites_the_file_with_the_answer_as_its_data() {
        let (origin, server) =
            serve(200, b"\xEF\xBB\xBF{\"b\":[1.0,2e3,{}],\"10\":\"\xC3\xA9\",\"2\":null}", 2);
        let source = format!("{origin}/moved");
        let content = snapshot_file(&snapshot(&source), fetch(&source), NOW).unwrap();
        assert_eq!(
            content,
            format!(
                "{{\n  \"source\": \"{source}\",\n  \"fetchedAt\": \"2023-11-14\",\n  \"data\": {{\n    \"b\": [\n      1.0,\n      2000.0,\n      {{}}\n    ],\n    \"10\": \"é\",\n    \"2\": null\n  }}\n}}\n"
            )
        );
        let seen = server.join().unwrap();
        assert!(seen.contains(&"get /moved http/1.1".to_string()), "{seen:?}");
        assert!(seen.contains(&"get /data http/1.1".to_string()), "{seen:?}");
        assert!(seen.contains(&"user-agent: docattest".to_string()), "{seen:?}");
        assert!(seen.contains(&"accept: */*".to_string()), "{seen:?}");
    }

    #[test]
    fn a_status_that_is_not_ok_is_the_users_to_fix() {
        let (origin, server) = serve(404, b"{}", 1);
        let source = format!("{origin}/data");
        assert_eq!(
            snapshot_file(&snapshot(&source), fetch(&source), NOW),
            Err(Error::User(format!("docattest: refresh regions: {source} answered 404")))
        );
        server.join().unwrap();
    }

    #[test]
    fn a_body_that_is_not_json_and_a_source_that_cannot_be_fetched_are_the_users_to_fix() {
        let (origin, server) = serve(200, b"<html>", 1);
        let source = format!("{origin}/data");
        let message = |result: Result<String>| match result {
            Err(Error::User(message)) => message,
            other => format!("{other:?}"),
        };
        let not_json = message(snapshot_file(&snapshot(&source), fetch(&source), NOW));
        assert!(
            not_json.starts_with(&format!("docattest: refresh regions: {source} did not answer JSON (")),
            "{not_json}"
        );
        server.join().unwrap();
        // The port is free again: nothing listens there.
        assert_eq!(
            message(snapshot_file(&snapshot(&source), fetch(&source), NOW)),
            format!("docattest: refresh regions: {source} did not answer")
        );
        assert_eq!(
            message(snapshot_file(&snapshot("not a url"), fetch("not a url"), NOW)),
            "docattest: refresh regions: not a url is not an http or https URL"
        );
        assert_eq!(
            message(snapshot_file(&snapshot("ftp://x/y"), fetch("ftp://x/y"), NOW)),
            "docattest: refresh regions: ftp://x/y is not an http or https URL"
        );
    }

    #[test]
    fn dates_are_utc() {
        assert_eq!(iso_date(0.0), "1970-01-01");
        assert_eq!(iso_date(1_709_251_199_999.0), "2024-02-29");
        assert_eq!(iso_date(1_709_251_200_000.0), "2024-03-01");
    }
}
