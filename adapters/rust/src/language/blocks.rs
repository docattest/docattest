//! How a block of a page is read as a Rust program.

/// What every generated program begins with: a snippet in prose is not held to the lints
/// a crate is, as rustdoc does not hold a doc test to them.
const HEADER: &str = "#![allow(unused)]\n";

/// A block as the Rust file it is compiled as.
///
/// A block that declares `fn main` is a whole program. Any other is the body of `main`,
/// as rustdoc reads a doc test: Rust lets a function hold `use` declarations and items,
/// so a snippet needs nothing more. A line that begins `# ` is part of the program and
/// hidden by renderers (rustdoc, mdBook); its marker is taken off.
pub struct Program {
    pub source: String,
    /// How many lines of the source come before the block's first line.
    pub offset: usize,
}

/// A block's code with the markers of its hidden lines taken off; the lines stay where
/// they are.
pub fn visible(code: &str) -> String {
    let shown: Vec<&str> = code
        .lines()
        .map(|line| match line.trim_start() {
            "#" => "",
            hidden if hidden.starts_with("# ") => &hidden[2..],
            escaped if escaped.starts_with("##") => &escaped[1..],
            _ => line,
        })
        .collect();
    shown.join("\n")
}

/// Whether a file declares `fn main` at its top level.
fn has_main(file: &syn::File) -> bool {
    file.items.iter().any(|item| matches!(item, syn::Item::Fn(f) if f.sig.ident == "main"))
}

/// Reads a block's code as a Rust program.
pub fn program(code: &str) -> Program {
    let code = visible(code);
    if syn::parse_file(&code).is_ok_and(|file| has_main(&file)) {
        return Program { source: format!("{HEADER}{code}\n"), offset: 1 };
    }
    Program { source: format!("{HEADER}fn main() {{\n{code}\n}}\n"), offset: 2 }
}

/// A block as a syntax tree, when it parses: as a whole program, or as the body of `main`.
pub fn parsed(code: &str) -> Option<syn::File> {
    syn::parse_file(&program(code).source).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snippet_is_the_body_of_main() {
        let read = program("use std::fmt;\nassert!(true);");
        assert_eq!(read.source, "#![allow(unused)]\nfn main() {\nuse std::fmt;\nassert!(true);\n}\n");
        assert_eq!(read.offset, 2);
    }

    #[test]
    fn a_block_with_a_main_is_a_whole_program() {
        let read = program("fn helper() {}\nfn main() {\n    helper();\n}");
        assert_eq!(read.source, "#![allow(unused)]\nfn helper() {}\nfn main() {\n    helper();\n}\n");
        assert_eq!(read.offset, 1);
    }

    #[test]
    fn hidden_lines_are_part_of_the_program_and_keep_their_place() {
        assert_eq!(
            visible("# use a::b;\n#\nlet x = 1;\n## not hidden\n#[derive(Debug)]"),
            "use a::b;\n\nlet x = 1;\n# not hidden\n#[derive(Debug)]"
        );
        assert!(parsed("# use std::fmt;\nlet x = 1;").is_some());
        assert!(parsed("let x = = 1;").is_none());
    }
}
