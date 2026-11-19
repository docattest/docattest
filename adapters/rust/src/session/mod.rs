//! The one project `open` creates, as the protocol sees it: what `open` says, and which
//! of the session's methods is answered from syntax (`analysis`) and which by
//! building and running programs (`running`).

use crate::analysis::Analysis;
use crate::analysis::surfaces::{Configured, workspace_start};
use crate::project::{self, Project};
use crate::protocol::{Answer, Attestation, Cases, Error, Example, Fact, decode};
use crate::running::{FactFiles, Limits, Runner, Snapshots};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The time limit of a program when `open` gives none.
const DEFAULT_TIMEOUT_MS: f64 = 10_000.0;

/// What `open` says.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Params {
    root: String,
    #[serde(default)]
    files: Option<HashMap<String, String>>,
    surfaces: Vec<Configured>,
    #[serde(default)]
    setup: Option<String>,
    timeout_ms: f64,
    workers: f64,
    network: bool,
    snapshots: Snapshots,
    #[serde(flatten)]
    fact_files: FactFiles,
    examples: Vec<Example>,
}

/// A project, for one docattest invocation.
struct Session {
    analysis: Arc<Analysis>,
    runner: Arc<Runner>,
    /// The directory a project in memory was written to, removed when the session closes.
    written: Option<PathBuf>,
}

/// Creates the session of a process.
pub fn open(params: Value) -> Result<Arc<dyn crate::protocol::Session>, Error> {
    let told: Params = decode(params)?;
    let (root, written) = match &told.files {
        Some(files) => {
            let dir = project::materialise(&told.root, files).map_err(|err| {
                Error::Internal(format!("cannot write the project in memory to a directory: {err}"))
            })?;
            (dir.clone(), Some(dir))
        }
        None => (canonical(&told.root)?, None),
    };
    let in_memory = told.files.as_ref().map(|_| told.root.as_str());
    let project = Project::read(&workspace_start(&root, &told.surfaces), in_memory);
    let analysis = Arc::new(Analysis::new(root, project, told.setup, &told.surfaces));
    let timeout_ms = if told.timeout_ms > 0.0 { told.timeout_ms } else { DEFAULT_TIMEOUT_MS };
    let limits = Limits { timeout_ms, workers: told.workers as usize, network: told.network };
    let runner = Arc::new(Runner::new(Arc::clone(&analysis), limits, told.snapshots, told.fact_files));
    // Building is most of the time a check takes, so it starts now: `compile` and `run`
    // find the examples `open` named already built, or wait for them.
    let early = Arc::clone(&runner);
    std::thread::spawn(move || early.build_ahead(&told.examples));
    Ok(Arc::new(Session { analysis, runner, written }))
}

/// The project's directory as the file system names it.
fn canonical(root: &str) -> Result<PathBuf, Error> {
    Path::new(root)
        .canonicalize()
        .map_err(|err| Error::Internal(format!("the project root {root} cannot be read: {err}")))
}

#[derive(Deserialize)]
struct ExamplesParams {
    examples: Vec<Example>,
}

#[derive(Deserialize)]
struct RunParams {
    examples: Vec<Example>,
    cases: Vec<Cases>,
}

#[derive(Deserialize)]
struct FactsParams {
    facts: Vec<Fact>,
}

#[derive(Deserialize)]
struct SymbolsParams {
    attestations: Vec<Attestation>,
}

#[derive(Deserialize)]
struct SurfaceParams {
    surface: String,
}

#[derive(Deserialize)]
struct FileParams {
    file: String,
}

#[derive(Deserialize)]
struct AffectedParams {
    base: HashMap<String, Option<String>>,
}

#[derive(Deserialize)]
struct InCodeParams {
    names: Vec<String>,
    files: Vec<String>,
}

impl crate::protocol::Session for Session {
    fn call(&self, method: &str, params: Value) -> Option<Answer> {
        let runner = &self.runner;
        Some(match method {
            "compile" => decode::<ExamplesParams>(params).map(|p| json!(runner.compile(&p.examples))),
            "run" => decode::<RunParams>(params).map(|p| json!(runner.run(&p.examples, &p.cases))),
            "evaluate" => decode::<FactsParams>(params).map(|p| json!(runner.evaluate(&p.facts))),
            "observables" => decode::<SurfaceParams>(params).and_then(|p| runner.observables(&p.surface)),
            _ => return self.read(method, params),
        })
    }

    fn close(&self) {
        // The examples `open` began building are let finish: cargo holds the files.
        self.runner.wait_for_build();
        if let Some(dir) = &self.written {
            // What is left of a project in memory is in the system's temporary
            // directory, and nothing reads it again.
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

impl Session {
    /// Answers a method that only reads the project's syntax; `None` for any other.
    fn read(&self, method: &str, params: Value) -> Option<Answer> {
        let analysis = &self.analysis;
        Some(match method {
            "symbols" => decode::<SymbolsParams>(params).map(|p| {
                let by_id = p.attestations.iter().map(|a| (a.id.clone(), json!(analysis.symbols(a))));
                Value::Object(by_id.collect())
            }),
            "exports" => decode::<SurfaceParams>(params).map(|p| json!(analysis.exports_of(&p.surface))),
            "describeSurfaces" => Ok(analysis.describe_surfaces()),
            "tests" => decode::<FileParams>(params).map(|p| analysis.list_tests(&p.file)),
            "affected" => decode::<AffectedParams>(params).map(|p| json!(analysis.affected(&p.base))),
            "inCode" => decode::<InCodeParams>(params).map(|p| json!(analysis.in_code(&p.names, &p.files))),
            _ => return None,
        })
    }
}
