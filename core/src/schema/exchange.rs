//! A whole conversation against the schemas: every response, and for each that has a result,
//! its request, the request's params and the result, by the request's method.

use serde_json::{Value, json};

use super::{METHODS, Schemas, plain};
use crate::adapter::client::is_blank_line;
use crate::json::parse_json;
use crate::text::prefix;

/// The file that holds the shape of a request and of a response.
const MESSAGES: &str = "messages.json";

/// How much of a line that is not JSON a problem quotes.
const QUOTED_CHARS: usize = 80;

impl Schemas {
    /// What does not fit the schemas in one conversation: `sent` the lines to the adapter,
    /// `received` the lines back. A line back that is not JSON is a problem, unless it is blank; a line sent that
    /// is not one may have been malformed on purpose, and so may the params of a request the
    /// adapter answered with an error: neither is held to a shape.
    pub fn check_exchange(&self, sent: &[String], received: &[String]) -> Vec<String> {
        let requests: Vec<Value> = sent.iter().filter_map(|line| parse_json(line).ok()).collect();
        let problems = |line: &String| match parse_json(line) {
            Ok(response) => self.response_problems(&response, &requests),
            Err(_) => vec![format!("a line on stdout is not JSON: {}", prefix(line, QUOTED_CHARS))],
        };
        received.iter().filter(|line| !is_blank_line(line.as_bytes())).flat_map(problems).collect()
    }

    /// A response against its schema and, when it has a result, its request's method's.
    fn response_problems(&self, response: &Value, sent: &[Value]) -> Vec<String> {
        let id = response.get("id").unwrap_or(&Value::Null);
        let mut out: Vec<String> = self
            .check(response, "Response", MESSAGES)
            .into_iter()
            .map(|problem| format!("response {}: {problem}", plain(id)))
            .collect();
        let request = sent.iter().find(|request| request.get("id") == Some(id));
        let (Some(request), Some(result)) = (request, response.get("result")) else { return out };
        let method = request.get("method").and_then(Value::as_str).unwrap_or_default();
        let result_schema = format!("{method}.result");
        if self.definition(&result_schema, METHODS).is_none() {
            return out;
        }
        let empty = json!({});
        let params = request.get("params").unwrap_or(&empty);
        let said = |what: &str, problems: Vec<String>| {
            problems.into_iter().map(move |problem| format!("{method} {what}: {problem}")).collect::<Vec<_>>()
        };
        out.extend(said("request", self.check(request, "Request", MESSAGES)));
        out.extend(said("params", self.check(params, &format!("{method}.params"), METHODS)));
        out.extend(said("result", self.check(result, &result_schema, METHODS)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_result_is_held_to_its_methods_schema_and_an_error_only_to_the_response_s() {
        let schemas = Schemas::embedded();
        let lines = |values: &[Value]| values.iter().map(Value::to_string).collect::<Vec<_>>();
        let sent = lines(&[
            json!({ "id": 1, "method": "hello", "params": { "protocol": 1 } }),
            json!({ "id": 2, "method": "assertions", "params": { "examples": 3 } }),
            json!({ "id": 3, "method": "exports", "params": { "surface": "default" } }),
        ]);
        let received = lines(&[
            json!({ "id": 2, "error": { "kind": "internal", "message": "examples is not a list" } }),
            json!({ "id": 3, "result": [1] }),
        ]);
        assert_eq!(
            schemas.check_exchange(&sent, &received),
            ["exports result: $: matches none of the alternatives ($[0]: 1 is not of type string)"]
        );
        assert_eq!(schemas.check_exchange(&sent, &lines(&[json!({ "id": 3 })])).len(), 1);
        assert_eq!(
            schemas.check_exchange(&sent, &["junk".to_string()]),
            ["a line on stdout is not JSON: junk"]
        );
        assert!(schemas.check_exchange(&sent, &[" \r".to_string()]).is_empty(), "a blank line is ignored");
    }
}
