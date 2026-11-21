//! `mutate` from end to end with a fake adapter, `tests/fixtures/mutate/adapter.sh`, whose
//! change of `src/k.txt` breaks the project (a stillborn mutant) and whose change of
//! `src/n.txt` fails what reads it.

use std::collections::HashSet;
use std::fs;

use super::run_mutate;
use super::verdict::{ALL_STILLBORN, Mutation, Verdict};
use crate::adapter::{Launch, Start};
use crate::files::source::DiskFiles;
use crate::git::tests::Temp;
use crate::project::check::Network;
use crate::project::state::launch;

/// The fake adapter, `tests/fixtures/mutate/adapter.sh`, read where the crate is built: the
/// crate as published carries no tests.
fn adapter_script() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mutate/adapter.sh");
    std::fs::read_to_string(path).unwrap()
}

const PAGE: &str = "N is `3`. <!-- fact: N -->\n\nM is `7`. <!-- fact: M -->\n\nF gives `2`. <!-- fact: F() -->\n\nJ is `5`. <!-- fact: J -->\n";

/// `mutate` on a project with `page`, with one copy at a time so the mutants run in order.
fn mutated(label: &str, page: &str) -> (Temp, Mutation) {
    let (project, adapter) = (Temp::new(label), Temp::new(&format!("{label}-adapter")));
    project.write(
        "docattest.config.json",
        r#"{"pages":["docs/*.md"],"surfaces":{"default":"x"},"adapter":"fake","runtime":{"workers":1}}"#,
    );
    project.write("docs/a.md", page);
    project.write("src/n.txt", "3\n");
    project.write("src/k.txt", "1\n");
    adapter.write("adapter.sh", &adapter_script());
    let words = ["sh".to_string(), adapter.0.join("adapter.sh").to_string_lossy().into_owned()];
    let launched = Launch::of_member(Some(&words), &adapter.0);
    let state = launch(DiskFiles::new(project.0.clone()), &launched, Start::Cold).unwrap().load(0.0).unwrap();
    let mutation = run_mutate(state, Network::Off, &HashSet::from(["docs/a.md".to_string()])).unwrap();
    (project, mutation)
}

#[test]
fn a_change_that_fails_a_proof_kills_it_one_that_breaks_the_project_is_stillborn() {
    let (project, mutation) = mutated("mutate-project", PAGE);
    let verdicts: Vec<(usize, &Verdict)> = mutation.proofs.iter().map(|p| (p.line, &p.verdict)).collect();
    assert_eq!(
        verdicts,
        [
            // Killed by the change of N; the change of K broke the project, and kills nothing.
            (
                1,
                &Verdict::Killed {
                    by: "src/n.txt:1 N: 3 → 4".into(),
                    tried: 1,
                    stillborn: 1,
                    ignored: vec![],
                    hidden_lines: 0,
                },
            ),
            // Run after the project was broken: it survives only if that change was undone.
            (
                3,
                &Verdict::Survived {
                    survivors: vec!["src/n.txt:1 N: 3 → 4".into()],
                    stillborn: 0,
                    hint: None,
                },
            ),
            (
                5,
                &Verdict::NotMutated {
                    why: "F is a function, class or type: what it gives is computed when it runs".into()
                }
            ),
            (7, &Verdict::NotMutated { why: ALL_STILLBORN.into() }),
        ]
    );
    assert_eq!((mutation.mutants, mutation.stillborn), (2, 1));
    for (file, text) in [("src/n.txt", "3\n"), ("src/k.txt", "1\n")] {
        assert_eq!(fs::read_to_string(project.0.join(file)).unwrap(), text);
    }
    let parent = project.0.parent().unwrap();
    let name = project.0.file_name().unwrap().to_string_lossy().into_owned();
    let copy = format!(".{name}.docattest-mutate-");
    let left = fs::read_dir(parent)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&copy));
    assert_eq!(left.count(), 0, "the copies are removed");
}

#[test]
fn a_proof_that_does_not_pass_as_it_is_is_not_mutated() {
    let (_, mutation) = mutated("mutate-failing", &PAGE.replace("`3`", "`5`"));
    assert_eq!(mutation.proofs[0].verdict, Verdict::NotMutated { why: "it does not pass as it is".into() });
}
