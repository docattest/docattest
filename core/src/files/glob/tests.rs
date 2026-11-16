use super::constants::length_error;
use super::translate::{RegexError, infidelity, translate};
use super::*;

const FIXTURE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/glob-cases.json"));

fn strings(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// The tally of a run over the picomatch fixture.
#[derive(Default)]
struct FixtureRun {
    paths: Vec<String>,
    failures: Vec<String>,
    exempted: std::collections::BTreeMap<String, usize>,
    cases: usize,
}

impl FixtureRun {
    fn check_set(&mut self, set: &serde_json::Value) {
        let globs = strings(&set["globs"]);
        if set["hangs"].as_bool() == Some(true) {
            self.cases += 1;
            if GlobMatcher::new(&globs).err().as_deref() != Some(HANGS) {
                self.failures.push(format!("{globs:?} hangs picomatch"));
            }
            return;
        }
        // `make_re` sees the glob as `GlobMatcher::new` passes it on: without a leading `./`.
        let first = globs.first().map(|g| w(g.strip_prefix("./").unwrap_or(g)));
        let first = first.as_deref().unwrap_or_default();
        self.check_source(set, &globs, first);
        self.check_validity(set, &globs, first);
        let exempt = self.exemption(&globs);
        let matcher = GlobMatcher::new(&globs);
        match set["error"].as_str() {
            Some(error) => {
                self.cases += 1;
                if matcher.as_ref().err().map(String::as_str) != Some(error) {
                    self.failures.push(format!("error {globs:?}: {:?} != {error:?}", matcher.err()));
                }
            }
            None => {
                let Ok(matcher) = matcher else {
                    self.failures.push(format!("{globs:?} failed: {:?}", matcher.err()));
                    return;
                };
                self.check_matches(set, &globs, &matcher, exempt.as_ref());
            }
        }
    }

    fn check_source(&mut self, set: &serde_json::Value, globs: &[String], first: &[u16]) {
        let expected_source: Option<U> = match (set["source"].as_str(), set["sourceUnits"].as_array()) {
            (Some(s), _) => Some(w(s)),
            (None, Some(units)) => {
                Some(units.iter().filter_map(|u| u.as_u64().and_then(|u| u16::try_from(u).ok())).collect())
            }
            _ => None,
        };
        if let Some(source) = expected_source {
            let ours = make_re(first);
            if ours.as_ref() != Ok(&source) {
                self.failures.push(format!(
                    "source {globs:?}: {:?} != {:?}",
                    ours.map(|s| String::from_utf16_lossy(&s)),
                    String::from_utf16_lossy(&source)
                ));
            }
        }
    }

    fn check_validity(&mut self, set: &serde_json::Value, globs: &[String], first: &[u16]) {
        if let Some(valid) = set["valid"].as_bool()
            && let Ok(source) = make_re(first)
        {
            let ours = translate(&source);
            if (ours != Err(RegexError::Syntax)) != valid {
                self.failures.push(format!("validity {globs:?}: {ours:?}, JS valid {valid}"));
            }
        }
    }

    /// The documented gaps (see the module docs): a lookbehind fancy_regex cannot run,
    /// or a backreference into a repeated group. Anything else must match exactly.
    fn exemption(&mut self, globs: &[String]) -> Option<String> {
        let mut exempt = None;
        for glob in globs {
            let Ok(source) = make_re(&w(glob.strip_prefix("./").unwrap_or(glob))) else {
                continue;
            };
            match infidelity(&source) {
                Some(reason) if reason == "lookbehind" || reason == "capture-reset" => {
                    exempt = Some(reason);
                }
                Some(reason) => self.failures.push(format!("{globs:?}: {reason}")),
                None => {}
            }
        }
        exempt
    }

    fn check_matches(
        &mut self,
        set: &serde_json::Value,
        globs: &[String],
        matcher: &GlobMatcher,
        exempt: Option<&String>,
    ) {
        let indices = set["p"].as_array().expect("p");
        let results = set["r"].as_str().expect("r").as_bytes();
        for (k, index) in indices.iter().enumerate() {
            self.cases += 1;
            let path = &self.paths[index.as_u64().expect("index") as usize];
            let expected = results[k] == b'1';
            if matcher.is_match(path) == expected {
                continue;
            }
            match exempt {
                Some(reason) => *self.exempted.entry(reason.clone()).or_insert(0) += 1,
                None => self.failures.push(format!("{globs:?} on {path:?}: expected {expected}")),
            }
        }
    }

    fn check_is_glob(&mut self, cases: &[serde_json::Value]) {
        for case in cases {
            self.cases += 1;
            let pattern = case[0].as_str().expect("pattern");
            let expected = case[1].as_bool().expect("bool");
            if is_glob(pattern) != expected {
                self.failures.push(format!("isGlob {pattern:?}: expected {expected}"));
            }
        }
    }
}

#[test]
fn matches_picomatch_fixture() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
    let paths = strings(&fixture["paths"]);
    let sets = fixture["sets"].as_array().expect("sets");
    let mut run = FixtureRun { paths, ..Default::default() };
    for set in sets {
        run.check_set(set);
    }
    run.check_is_glob(fixture["isGlob"].as_array().expect("isGlob"));
    let FixtureRun { failures, exempted, cases, .. } = run;
    assert!(cases > 10_000, "only {cases} cases");
    println!("{cases} cases; differences in documented gaps: {exempted:?}");
    assert!(
        failures.is_empty(),
        "{} of {cases} cases differ from picomatch:\n{}",
        failures.len(),
        failures.iter().take(60).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn common_globs() {
    let m = |globs: &[&str]| {
        GlobMatcher::new(&globs.iter().map(|g| g.to_string()).collect::<Vec<_>>()).expect("valid")
    };
    let docs = m(&["./docs/**/*.md"]);
    assert!(docs.is_match("docs/a.md"));
    assert!(docs.is_match("docs/deep/.hidden/b.md"));
    assert!(!docs.is_match("src/a.md"));
    assert!(m(&["**/*.{md,mdx}"]).is_match(".github/x.mdx"));
    assert!(!m(&[]).is_match("a"));
    assert!(m(&["!docs/**"]).is_match("src/a.ts"));
    assert!(!m(&["?"]).is_match("😀"));
    assert!(m(&["??"]).is_match("😀"));
}

#[test]
fn errors() {
    let e = |g: &str| GlobMatcher::new(&[g.to_string()]).err();
    assert_eq!(e("").as_deref(), Some(EMPTY_PATTERN));
    assert_eq!(e("./").as_deref(), Some(EMPTY_PATTERN));
    assert_eq!(e("{a..*(}|)").as_deref(), Some(UNDEFINED_TYPE));
    assert_eq!(e("x/\\\\\\\\").as_deref(), Some(HANGS));
    let long = "a/".repeat(MAX_LENGTH);
    assert_eq!(e(&long), Some(length_error(MAX_LENGTH * 2)));
}

#[test]
fn is_glob_basics() {
    assert!(is_glob("docs/*.md"));
    assert!(is_glob("{a,b}"));
    assert!(!is_glob("README.md"));
    assert!(!is_glob("\\*"));
}
