//! Recorded conversations ([`super::record`]) held to the schemas.

use std::io::Write;

use crate::json::parse_json;
use crate::schema::Schemas;
use crate::text::prefix;

/// How much of a problem is printed.
const PROBLEM_CHARS: usize = 400;

/// One conversation of a log: the lines to the adapter, and the lines back.
#[derive(Default)]
struct Exchange {
    sent: Vec<String>,
    received: Vec<String>,
}

/// Holds every log in `logs` (path and text) to the schemas, printing what does not fit and
/// then the count. Whether there were lines, and every one fits.
pub fn validate_logs(logs: &[(String, String)], out: &mut dyn Write) -> Result<bool, String> {
    let schemas = Schemas::embedded();
    let (mut lines, mut bad) = (0, 0);
    for (path, text) in logs {
        let (exchanges, count) = conversations(text).map_err(|why| format!("{path}: {why}"))?;
        lines += count;
        let problems =
            exchanges.iter().flat_map(|exchange| schemas.check_exchange(&exchange.sent, &exchange.received));
        for problem in problems {
            bad += 1;
            writeln!(out, "✗ {path}: {}", prefix(&problem, PROBLEM_CHARS)).map_err(unwritten)?;
        }
    }
    writeln!(out, "schemas: {lines} recorded lines, {bad} that do not fit").map_err(unwritten)?;
    Ok(bad == 0 && lines > 0)
}

/// A log's conversations, and how many lines it holds: a new conversation begins with each
/// `hello` request, where ids start again.
fn conversations(text: &str) -> Result<(Vec<Exchange>, usize), String> {
    let mut exchanges: Vec<Exchange> = Vec::new();
    let mut lines = 0;
    for recorded in text.split('\n').filter(|line| !line.is_empty()) {
        let value = parse_json(recorded).map_err(|why| format!("not a recorded line: {why}"))?;
        let (Some(to), Some(line)) = (value["to"].as_str(), value["line"].as_str()) else {
            return Err(format!("not a recorded line: {recorded}"));
        };
        let to_adapter = to == "adapter";
        if to_adapter && is_hello(line) {
            exchanges.push(Exchange::default());
        }
        if let Some(exchange) = exchanges.last_mut() {
            let side = if to_adapter { &mut exchange.sent } else { &mut exchange.received };
            side.push(line.to_string());
        }
        lines += 1;
    }
    Ok((exchanges, lines))
}

fn is_hello(request: &str) -> bool {
    parse_json(request).is_ok_and(|request| request["method"] == "hello")
}

fn unwritten(error: std::io::Error) -> String {
    format!("conformance validate: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validated(log: &str) -> (bool, String) {
        let mut out = Vec::new();
        let fits = validate_logs(&[("a.jsonl".into(), log.into())], &mut out).unwrap();
        (fits, String::from_utf8(out).unwrap())
    }

    #[test]
    fn each_hello_starts_a_conversation_whose_answers_are_held_to_the_schemas() {
        let log = [
            r#"{"to":"adapter","line":"{\"id\":1,\"method\":\"hello\",\"params\":{\"protocol\":1}}"}"#,
            r#"{"to":"adapter","line":"{\"id\":2,\"method\":\"exports\",\"params\":{\"surface\":\"default\"}}"}"#,
            r#"{"to":"core","line":"{\"id\":2,\"error\":{\"kind\":\"internal\",\"message\":\"no session\"}}"}"#,
            r#"{"to":"adapter","line":"{\"id\":1,\"method\":\"hello\",\"params\":{\"protocol\":1}}"}"#,
            r#"{"to":"core","line":"not json"}"#,
        ]
        .join("\n");
        let (fits, said) = validated(&log);
        assert!(!fits);
        assert_eq!(
            said,
            "✗ a.jsonl: a line on stdout is not JSON: not json\nschemas: 5 recorded lines, 1 that do not fit\n"
        );
        assert_eq!(validated(""), (false, "schemas: 0 recorded lines, 0 that do not fit\n".to_string()));
    }
}
