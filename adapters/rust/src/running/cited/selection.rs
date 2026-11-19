//! Which run of `cargo test` runs a cited test, and which of the tests it lists are the
//! cited one's: decided from the project's targets and source, with nothing run.

use crate::analysis::modules::{Crate, Read};
use crate::analysis::tests::Printed;
use crate::project::{Target, TargetKind};
use std::path::{Path, PathBuf};

/// The module files of a project's targets, each with its module's path in its target.
pub(super) struct Modules<'a> {
    files: Vec<(&'a Target, PathBuf, Vec<String>)>,
}

impl<'a> Modules<'a> {
    /// Follows each target's `mod` declarations from its root file.
    pub(super) fn read(targets: &'a [Target], read: Read) -> Self {
        let mut files: Vec<(&'a Target, PathBuf, Vec<String>)> = Vec::new();
        for target in targets {
            for module in Crate::load(&target.name, &target.root, read).modules {
                // A module written in place shares the file of the one around it, which
                // comes first and is the file's own.
                let known =
                    files.iter().any(|(by, file, _)| std::ptr::eq(*by, target) && *file == module.file);
                if !known {
                    files.push((target, module.file, module.path));
                }
            }
        }
        Modules { files }
    }

    /// The one target a file is a module of, with the file's module path in it. `shown` is
    /// the file as a message names it.
    pub(super) fn owner(&self, file: &Path, shown: &str) -> Result<(&'a Target, &[String]), String> {
        let owners: Vec<_> = self.files.iter().filter(|(_, path, _)| path == file).collect();
        match owners.as_slice() {
            [(target, _, module)] => Ok((target, module)),
            [] => Err(format!("{shown} is not a module of any target cargo test builds")),
            several => {
                let named: Vec<String> =
                    several.iter().map(|(target, _, _)| selects(target).join(" ")).collect();
                Err(format!(
                    "{shown} is a module of several targets ({}), and one run of cargo test selects one",
                    named.join("; ")
                ))
            }
        }
    }
}

/// The arguments of `cargo test` that select a target and nothing else.
pub(super) fn selects(target: &Target) -> Vec<String> {
    let mut args = vec!["--package".to_string(), target.package.clone()];
    let named = |flag: &str| [flag.to_string(), target.name.clone()];
    match target.kind {
        TargetKind::Lib => args.push("--lib".to_string()),
        TargetKind::Bin => args.extend(named("--bin")),
        TargetKind::Test => args.extend(named("--test")),
        TargetKind::Bench => args.extend(named("--bench")),
        TargetKind::Example => args.extend(named("--example")),
    }
    args
}

/// The arguments of `cargo` that list a target's tests.
pub(super) fn listing(target: &Target) -> Vec<String> {
    command(target, ["--list".to_string()])
}

/// The arguments of `cargo` that run exactly some tests of a target, by the names its
/// listing gives them. They are arguments of their own, never read by a shell.
pub(super) fn running(target: &Target, names: &[String]) -> Vec<String> {
    command(target, std::iter::once("--exact".to_string()).chain(names.iter().cloned()))
}

fn command(target: &Target, harness: impl IntoIterator<Item = String>) -> Vec<String> {
    let cargo = std::iter::once("test".to_string()).chain(selects(target));
    cargo.chain(std::iter::once("--".to_string())).chain(harness).collect()
}

/// The names of a listing that are a cited test's tests, given the module path of the
/// test's file.
pub(super) fn selected(listed: &[String], module: &[String], printed: &Printed) -> Vec<String> {
    let within = |name: &String| {
        let path: Vec<&str> = name.split("::").collect();
        let in_file = path.len() > module.len() && module.iter().zip(&path).all(|(own, shown)| own == shown);
        in_file && printed.includes(&path[module.len()..])
    };
    listed.iter().filter(|name| within(name)).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn target(kind: TargetKind, name: &str, root: &str) -> Target {
        Target { package: "shop".into(), name: name.into(), kind, root: root.into() }
    }

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn each_kind_of_target_is_selected_by_its_own_flag() {
        let selected = |kind, name| selects(&target(kind, name, "/x.rs")).join(" ");
        assert_eq!(selected(TargetKind::Lib, "shop"), "--package shop --lib");
        assert_eq!(selected(TargetKind::Bin, "shopd"), "--package shop --bin shopd");
        assert_eq!(selected(TargetKind::Test, "price-rules"), "--package shop --test price-rules");
        assert_eq!(selected(TargetKind::Bench, "speed"), "--package shop --bench speed");
        assert_eq!(selected(TargetKind::Example, "demo"), "--package shop --example demo");
    }

    #[test]
    fn a_run_lists_then_runs_exactly_the_names_it_listed() {
        let lib = target(TargetKind::Lib, "shop", "/x.rs");
        assert_eq!(listing(&lib), words("test --package shop --lib -- --list"));
        let names = words("tests::adds tests::rounds::test_case_1_a_quarter_off");
        assert_eq!(
            running(&lib, &names),
            words(
                "test --package shop --lib -- --exact tests::adds tests::rounds::test_case_1_a_quarter_off"
            )
        );
    }

    #[test]
    fn a_file_belongs_to_the_target_whose_modules_lead_to_it() {
        let files = HashMap::from([
            ("/p/src/lib.rs", "mod money;\nmod tests { #[test] fn adds() {} }"),
            ("/p/src/money/mod.rs", "mod rounding;"),
            ("/p/src/money/rounding.rs", ""),
            ("/p/tests/a.rs", "mod common;"),
            ("/p/tests/b.rs", "mod common;"),
            ("/p/tests/common.rs", ""),
            ("/p/tests/lonely.rs", ""),
        ]);
        let read = |path: &Path| files.get(path.to_str().unwrap()).map(|text| text.to_string());
        let targets = [
            target(TargetKind::Lib, "shop", "/p/src/lib.rs"),
            target(TargetKind::Test, "a", "/p/tests/a.rs"),
            target(TargetKind::Test, "b", "/p/tests/b.rs"),
        ];
        let modules = Modules::read(&targets, &read);
        let owner = |file: &str| {
            modules
                .owner(Path::new(file), file)
                .map(|(target, module)| (target.name.clone(), module.join("::")))
        };
        assert_eq!(owner("/p/src/lib.rs"), Ok(("shop".into(), String::new())));
        assert_eq!(owner("/p/src/money/rounding.rs"), Ok(("shop".into(), "money::rounding".into())));
        assert_eq!(owner("/p/tests/a.rs"), Ok(("a".into(), String::new())));
        assert_eq!(
            owner("/p/tests/lonely.rs"),
            Err("/p/tests/lonely.rs is not a module of any target cargo test builds".into())
        );
        assert!(
            owner("/p/tests/common.rs")
                .unwrap_err()
                .contains("--package shop --test a; --package shop --test b")
        );
    }

    #[test]
    fn the_names_selected_are_below_the_files_module() {
        let listed =
            words("money::adds money::rounding::adds money::rounding::adds_more shop::rounding::adds");
        let printed = Printed::Function(words("adds"));
        assert_eq!(selected(&listed, &words("money rounding"), &printed), ["money::rounding::adds"]);
        assert_eq!(selected(&listed, &[], &printed), Vec::<String>::new());
    }
}
