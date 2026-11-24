//! Commands merged into others, by their old names. Asked for one, docattest says which
//! command does its work now, so a script or an old page that names it says how to move on.

/// Each old command name, and the command that does its work now, as a user writes it.
const MERGED: [(&str, &str); 5] = [
    ("index", "docattest where"),
    ("inventory", "docattest audit --inventory"),
    ("agents", "docattest init --agents"),
    ("hooks", "docattest init --hooks (or init --git-hook for git's pre-commit hook)"),
    ("skill", "docattest init --skill"),
];

/// Why `command` is not one: the command that took its work over, else that it is unknown,
/// with the usage.
pub(super) fn unknown_command(command: &str, usage: &str) -> String {
    match MERGED.iter().find(|(old, _)| *old == command) {
        Some((_, now)) => format!("\"{command}\" was merged into another command: run {now}"),
        None => format!("unknown command \"{command}\"\n\n{usage}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_merged_command_names_the_one_that_took_it_over() {
        assert_eq!(
            unknown_command("index", "USAGE"),
            "\"index\" was merged into another command: run docattest where"
        );
        assert_eq!(
            unknown_command("hooks", "USAGE"),
            "\"hooks\" was merged into another command: run docattest init --hooks (or init \
             --git-hook for git's pre-commit hook)"
        );
    }

    #[test]
    fn any_other_unknown_command_gets_the_usage() {
        assert_eq!(unknown_command("bogus", "USAGE"), "unknown command \"bogus\"\n\nUSAGE");
    }
}
