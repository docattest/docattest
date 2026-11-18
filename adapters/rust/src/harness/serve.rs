//! Serving a generated program's items one by one: fact expressions, what a surface
//! offers, the rows of a table.
//!
//! The adapter starts the program with three arguments: the file of its job, the file
//! its results go to (a file, so that nothing the program prints can be taken for a
//! result), and the item to begin at. Each item gets the job's time limit. One that
//! panics is reported and the next one runs; one that runs out of time cannot be
//! stopped, so the program reports it and ends, and the adapter starts it again after it.

use super::json::{self, Json, quote};
use super::observe::Observation;
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::Duration;

/// One thing a program serves: it returns its result as JSON text.
pub type Item = Box<dyn Fn() -> String + Send + Sync>;

/// Something that reads a value when it is asked to.
pub type Reader = Box<dyn Fn() -> Observation + Send + Sync>;

/// A row of a cases table: its cells by column.
pub type Row = HashMap<String, String>;

/// A runner of a cases table: `Ok` when the row holds, else why it does not.
pub type Runner = fn(&Row) -> Result<(), String>;

static JOB: OnceLock<Json> = OnceLock::new();
static PANIC: Mutex<Option<String>> = Mutex::new(None);

fn stop(why: &str) -> ! {
    eprintln!("docattest: {why}");
    std::process::exit(2);
}

/// What the adapter told this program.
fn job() -> &'static Json {
    JOB.get_or_init(|| {
        let path =
            std::env::args().nth(1).unwrap_or_else(|| stop("usage: <job.json> <results> <first item>"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|err| stop(&err.to_string()));
        json::parse(&text).unwrap_or_else(|err| stop(&err))
    })
}

fn write(line: String) {
    let path = std::env::args().nth(2).unwrap_or_else(|| stop("no results file"));
    let file = std::fs::OpenOptions::new().append(true).create(true).open(path);
    let written = file.and_then(|mut file| writeln!(file, "{line}"));
    written.unwrap_or_else(|err| stop(&err.to_string()));
}

/// Keeps a panic's message for the item that panicked, in place of printing it.
fn quiet_panics() {
    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned());
        *PANIC.lock().unwrap_or_else(|p| p.into_inner()) = Some(message.unwrap_or_else(|| "a panic".into()));
    }));
}

/// Runs the items from the one the adapter names, and writes a line for each.
pub fn serve(items: Vec<Item>) {
    let first = std::env::args().nth(3).and_then(|n| n.parse().ok()).unwrap_or(0);
    let limit = Duration::from_secs_f64(job()["timeoutMs"].number() / 1000.0);
    quiet_panics();
    let items = Arc::new(items);
    for k in first..items.len() {
        let (done, result) = mpsc::channel();
        let shared = Arc::clone(&items);
        std::thread::spawn(move || {
            // The item is run once and its state is not looked at again after a panic.
            let _ = done.send(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| shared[k]())));
        });
        match result.recv_timeout(limit) {
            Ok(Ok(value)) => write(format!("{{\"item\":{k},\"value\":{value}}}")),
            Ok(Err(_)) => {
                let message = PANIC.lock().unwrap_or_else(|p| p.into_inner()).take().unwrap_or_default();
                write(format!("{{\"item\":{k},\"panic\":{}}}", quote(&message)));
            }
            Err(_) => {
                write(format!("{{\"item\":{k},\"timeout\":true}}"));
                std::process::exit(3);
            }
        }
    }
}

/// Serves fact expressions: each item is the Observation of one.
pub fn facts(facts: Vec<Reader>) {
    serve(facts.into_iter().map(|fact| Box::new(move || fact().to_json()) as Item).collect());
}

/// Serves one item: the values a surface offers, each by the expression that reads it.
pub fn observables(roots: Vec<(&'static str, Reader)>) {
    serve(vec![Box::new(move || {
        let found: Vec<String> = roots
            .iter()
            .map(|(name, read)| {
                format!("{{\"expression\":{0},\"path\":[{0}],\"value\":{1}}}", quote(name), read().to_json())
            })
            .collect();
        format!("[{}]", found.join(","))
    })]);
}

/// The data of a snapshot of the project. A fact expression calls it by this name.
pub fn snapshot(name: &str) -> Json {
    let snapshots = &job()["snapshots"];
    if !snapshots["data"].keys().contains(&name) {
        let dir = snapshots["dir"].as_str().unwrap_or("(no snapshots dir configured)");
        panic!("snapshot \"{name}\" not found in {dir}");
    }
    snapshots["data"][name].clone()
}

/// The data of a manifest of the project, by the path a fact writes. A fact expression
/// calls it by this name. The core reads a manifest only when a fact writes its path out;
/// one built at run time was never read.
pub fn manifest(path: &str) -> Json {
    let manifests = &job()["manifests"];
    if !manifests.keys().contains(&path) {
        panic!("manifest \"{path}\" was not read: name it in the fact itself, as manifest(\"{path}\")");
    }
    manifests[path].clone()
}

/// The text of a file of the project, by the path a fact writes, so that a fact reads it
/// with the methods of `str`: `file("Dockerfile").lines().find_map(|l| l.strip_prefix("USER "))`.
/// A fact expression calls it by this name. The core reads a file only when a fact writes
/// its path out, as it does a manifest.
pub fn file(path: &str) -> String {
    let Some(text) = job()["texts"][path].as_str() else {
        panic!("file \"{path}\" was not read: name it in the fact itself, as file(\"{path}\")");
    };
    text.to_string()
}

/// Serves the rows of the job's table with the runner the setup module names for it:
/// each item is "" for a row that holds, else why it does not. `columns` are the columns
/// each runner says its table must have.
pub fn cases(runners: &[(&str, Runner)], columns: &[(&str, &[&str])]) {
    let table = &job()["cases"];
    let name = table["runner"].as_str().unwrap_or("");
    let found = runners.iter().find(|(runner, _)| *runner == name);
    let declared = columns.iter().find(|(runner, _)| *runner == name).map_or(&[][..], |(_, columns)| columns);
    let Some((_, run)) = found.filter(|_| missing(table, declared).is_empty()) else {
        return write(format!(
            "{{\"failed\":{}}}",
            quote(&refusal(table, runners, found.is_some(), declared))
        ));
    };
    let run = *run;
    let rows: Vec<Row> = table["rows"].iter().map(row).collect();
    serve(
        rows.into_iter()
            .map(|row| Box::new(move || quote(&run(&row).err().unwrap_or_default())) as Item)
            .collect(),
    );
}

fn row(told: &Json) -> Row {
    let values = &told["values"];
    values.keys().into_iter().map(|key| (key.to_string(), values[key].to_string())).collect()
}

/// The declared columns a table does not have.
fn missing(table: &Json, declared: &[&str]) -> Vec<String> {
    let header: Vec<String> = table["header"].iter().map(Json::to_string).collect();
    declared
        .iter()
        .filter(|column| !header.iter().any(|h| h == *column))
        .map(|column| format!("\"{column}\""))
        .collect()
}

/// Why a table cannot be run at all.
fn refusal(table: &Json, runners: &[(&str, Runner)], found: bool, declared: &[&str]) -> String {
    let (name, line) = (table["runner"].to_string(), table["line"].number());
    if !found {
        let mut known: Vec<&str> = runners.iter().map(|(runner, _)| *runner).collect();
        known.sort_unstable();
        let listed = if known.is_empty() { "none".to_string() } else { known.join(", ") };
        return format!(
            "cases runner \"{name}\" at L{line} is not in CASES of {} (known: {listed})",
            table["setup"]
        );
    }
    let missing = missing(table, declared);
    let noun = if missing.len() > 1 { "columns" } else { "column" };
    format!(
        "cases \"{name}\" at L{line}: the table is missing {noun} {} declared by the runner",
        missing.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Json {
        json::parse(r#"{"runner":"nope","setup":"docs/setup.rs","line":7,"header":["a"],"rows":[{"values":{"a":"1","b":"x y"}}]}"#)
            .unwrap()
    }

    #[test]
    fn a_table_that_cannot_run_says_why() {
        let runners: [(&str, Runner); 2] = [("subtracts", |_| Ok(())), ("adds", |_| Ok(()))];
        assert_eq!(
            refusal(&table(), &runners, false, &[]),
            "cases runner \"nope\" at L7 is not in CASES of docs/setup.rs (known: adds, subtracts)"
        );
        assert!(
            refusal(&table(), &runners, true, &["a", "b", "sum"])
                .ends_with("is missing columns \"b\", \"sum\" declared by the runner")
        );
        assert!(missing(&table(), &["a"]).is_empty());
    }

    #[test]
    fn a_row_is_its_cells_by_column() {
        let cells = row(&table()["rows"][0]);
        assert_eq!((cells["a"].as_str(), cells["b"].as_str()), ("1", "x y"));
    }
}
