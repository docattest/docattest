//! The changes to the literals of a declared value, each on its own, in source order: a
//! number gains one, a string an `x` before its closing quote, a boolean flips, and a
//! slice or a `vec!` loses its last item before its items are changed.
//!
//! What runs only when it is called (a closure, a function the value declares) is left
//! alone, and so are types, patterns and attributes, which hold no value: changing an
//! array's length there, or in `[x; N]`, would not compile against its declared type.

use super::offsets::Offsets;
use proc_macro2::extra::DelimSpan;
use std::ops::Range;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, Lit, Token, Type};

/// One change to a file's text.
#[derive(Debug, PartialEq)]
pub(super) struct Change {
    pub(super) range: Range<usize>,
    pub(super) replacement: String,
    pub(super) description: String,
}

/// The items of a slice or a `vec!`.
type Items = Punctuated<Expr, Token![,]>;

/// The changes to the literals of a value declared with a type, in source order. When no
/// literal is present, a fallback expression change is offered for numeric and boolean
/// primitive constants.
pub(super) fn changes(value: &Expr, declared: &Type, offsets: &Offsets) -> Vec<Change> {
    let mut fixed = Vec::new();
    fixed_arrays(value, declared, offsets, &mut fixed);
    let mut literals = Literals { offsets, fixed, found: Vec::new() };
    literals.visit_expr(value);
    if literals.found.is_empty()
        && let Some(change) = expression_change(value, declared, offsets)
    {
        literals.found.push(change);
    }
    literals.found
}

/// Fallback change for a primitive scalar constant initialized without a literal (such as
/// `const FALLBACK: u16 = config::PORT;`).
fn expression_change(value: &Expr, declared: &Type, offsets: &Offsets) -> Option<Change> {
    if !is_pure_non_literal(value) {
        return None;
    }
    let written = offsets.text(value.span());
    let wrapped = match value {
        Expr::Path(_) | Expr::Field(_) | Expr::Paren(_) => written.to_string(),
        _ => format!("({written})"),
    };
    let replacement = match primitive_type(declared)? {
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "isize" => {
            format!("{wrapped} + 1")
        }
        "f32" | "f64" => format!("{wrapped} + 1.0"),
        "bool" => format!("!{wrapped}"),
        _ => return None,
    };
    let description = format!("{written} → {replacement}");
    Some(Change { range: offsets.range(value.span()), replacement, description })
}

fn is_pure_non_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Path(_) => true,
        Expr::Field(field) => is_pure_non_literal(&field.base),
        Expr::Paren(paren) => is_pure_non_literal(&paren.expr),
        Expr::Unary(unary) => is_pure_non_literal(&unary.expr),
        Expr::Binary(binary) => is_pure_non_literal(&binary.left) && is_pure_non_literal(&binary.right),
        _ => false,
    }
}

fn primitive_type(ty: &Type) -> Option<&str> {
    match ty {
        Type::Paren(paren) => primitive_type(&paren.elem),
        Type::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            let segment = path.path.segments.first()?;
            segment.arguments.is_empty().then(|| primitive_name(&segment.ident))?
        }
        _ => None,
    }
}

fn primitive_name(ident: &syn::Ident) -> Option<&'static str> {
    const NAMES: [&str; 15] = [
        "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "isize", "f32", "f64", "bool",
        "",
    ];
    NAMES.iter().copied().find(|&name| !name.is_empty() && ident == name)
}

/// Collects the changes of the literals it visits.
struct Literals<'o, 't> {
    offsets: &'o Offsets<'t>,
    /// Where the arrays whose length the declared type writes open: one of them loses no
    /// item, even behind a reference.
    fixed: Vec<usize>,
    found: Vec<Change>,
}

impl<'ast> Visit<'ast> for Literals<'_, '_> {
    fn visit_lit(&mut self, lit: &'ast Lit) {
        let written = self.offsets.text(lit.span());
        if let Some(replacement) = changed(lit, written) {
            let description = format!("{written} → {replacement}");
            self.found.push(Change { range: self.offsets.range(lit.span()), replacement, description });
        }
    }

    fn visit_expr_reference(&mut self, reference: &'ast syn::ExprReference) {
        if let Expr::Array(array) = &*reference.expr {
            let brackets = &array.bracket_token.span;
            if !self.fixed.contains(&self.offsets.range(brackets.open()).start) {
                self.remove_last(&array.elems, brackets);
            }
        }
        visit::visit_expr_reference(self, reference);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        // The items of a macro are tokens to the parser; those of `vec!` are expressions.
        if !mac.path.is_ident("vec") {
            return;
        }
        let Ok(items) = mac.parse_body_with(Items::parse_terminated) else { return };
        self.remove_last(&items, delimiters(&mac.delimiter));
        items.iter().for_each(|item| self.visit_expr(item));
    }

    fn visit_expr_repeat(&mut self, repeat: &'ast syn::ExprRepeat) {
        self.visit_expr(&repeat.expr);
    }

    fn visit_item(&mut self, item: &'ast syn::Item) {
        match item {
            syn::Item::Const(constant) => self.visit_expr(&constant.expr),
            syn::Item::Static(variable) => self.visit_expr(&variable.expr),
            _ => {}
        }
    }

    fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}

    fn visit_type(&mut self, _: &'ast Type) {}

    fn visit_generic_argument(&mut self, _: &'ast syn::GenericArgument) {}

    fn visit_pat(&mut self, _: &'ast syn::Pat) {}

    fn visit_attribute(&mut self, _: &'ast syn::Attribute) {}
}

impl Literals<'_, '_> {
    /// Offers the last of some items removed: with the comma before it, or, when it is the
    /// only one, everything between the brackets, so a trailing comma goes too.
    fn remove_last(&mut self, items: &Items, brackets: &DelimSpan) {
        let Some(last) = items.last() else { return };
        let start = match items.len() {
            1 => self.offsets.range(brackets.open()).end,
            n => self.offsets.range(items[n - 2].span()).end,
        };
        let end = match items.len() {
            1 => self.offsets.range(brackets.close()).start,
            _ => self.offsets.range(last.span()).end,
        };
        let shown = self.offsets.text(last.span()).split_whitespace().collect::<Vec<_>>().join(" ");
        let description = format!("{shown} removed");
        self.found.push(Change { range: start..end, replacement: String::new(), description });
    }
}

/// A literal as it is changed, from its text as written; `None` for one left alone (a
/// character, a byte) or one that has no next value of its type.
fn changed(lit: &Lit, written: &str) -> Option<String> {
    match lit {
        Lit::Int(int) => {
            let next = int.base10_digits().parse::<u128>().ok()?.checked_add(1)?;
            Some(format!("{next}{}", int.suffix()))
        }
        Lit::Float(float) => {
            // Debug writes an f64 as Rust reads a float back: `2.0`, never `2`.
            let next = float.base10_digits().parse::<f64>().ok()? + 1.0;
            next.is_finite().then(|| format!("{next:?}{}", float.suffix()))
        }
        Lit::Str(_) | Lit::ByteStr(_) | Lit::CStr(_) => Some(with_x(written)),
        Lit::Bool(flag) => Some((!flag.value).to_string()),
        _ => None,
    }
}

/// A string literal with an `x` before its closing quote, after any `#` of a raw string.
fn with_x(written: &str) -> String {
    let quote = written.trim_end_matches('#').len() - 1;
    format!("{}x{}", &written[..quote], &written[quote..])
}

/// The brackets, parentheses or braces a macro's items are written between.
fn delimiters(delimiter: &syn::MacroDelimiter) -> &DelimSpan {
    match delimiter {
        syn::MacroDelimiter::Paren(paren) => &paren.span,
        syn::MacroDelimiter::Brace(brace) => &brace.span,
        syn::MacroDelimiter::Bracket(bracket) => &bracket.span,
    }
}

/// Collects where the arrays of a value open whose length its type writes (`[T; N]`,
/// `&[T; N]`), following the value and its type together as far as they go side by side.
fn fixed_arrays(value: &Expr, ty: &Type, offsets: &Offsets, out: &mut Vec<usize>) {
    match (value, ty) {
        (Expr::Paren(inner), _) => fixed_arrays(&inner.expr, ty, offsets, out),
        (_, Type::Paren(inner)) => fixed_arrays(value, &inner.elem, offsets, out),
        (Expr::Reference(reference), Type::Reference(referent)) => {
            fixed_arrays(&reference.expr, &referent.elem, offsets, out);
        }
        (Expr::Tuple(tuple), Type::Tuple(types)) => {
            tuple.elems.iter().zip(&types.elems).for_each(|(item, of)| fixed_arrays(item, of, offsets, out));
        }
        (Expr::Array(array), Type::Array(of)) => {
            out.push(offsets.range(array.bracket_token.span.open()).start);
            array.elems.iter().for_each(|item| fixed_arrays(item, &of.elem, offsets, out));
        }
        (Expr::Array(array), Type::Slice(of)) => {
            array.elems.iter().for_each(|item| fixed_arrays(item, &of.elem, offsets, out));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each change to the value of the first constant or static of a file, as its
    /// description and the file with it applied, which must still parse.
    fn mutated(source: &str) -> Vec<(String, String)> {
        let file = syn::parse_file(source).unwrap();
        let (ty, value) = file
            .items
            .iter()
            .find_map(|item| match item {
                syn::Item::Const(constant) => Some((&*constant.ty, &*constant.expr)),
                syn::Item::Static(variable) => Some((&*variable.ty, &*variable.expr)),
                _ => None,
            })
            .unwrap();
        let applied = |change: Change| {
            let mut edited = source.to_string();
            edited.replace_range(change.range, &change.replacement);
            assert!(syn::parse_file(&edited).is_ok(), "{edited}");
            (change.description, edited)
        };
        changes(value, ty, &Offsets::new(source)).into_iter().map(applied).collect()
    }

    fn expected(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(description, edited)| (description.to_string(), edited.to_string())).collect()
    }

    #[test]
    fn an_integer_gains_one_in_decimal_and_keeps_its_suffix() {
        assert_eq!(
            mutated("const A: (u16, u32, u32, u8) = (8080, 3u32, 10_000, 0x1F);"),
            expected(&[
                ("8080 → 8081", "const A: (u16, u32, u32, u8) = (8081, 3u32, 10_000, 0x1F);"),
                ("3u32 → 4u32", "const A: (u16, u32, u32, u8) = (8080, 4u32, 10_000, 0x1F);"),
                ("10_000 → 10001", "const A: (u16, u32, u32, u8) = (8080, 3u32, 10001, 0x1F);"),
                ("0x1F → 32", "const A: (u16, u32, u32, u8) = (8080, 3u32, 10_000, 32);"),
            ])
        );
    }

    #[test]
    fn a_float_gains_one_written_as_a_float_and_keeps_its_suffix() {
        assert_eq!(
            mutated("const F: (f64, f32, f32, f64) = (1.5, 2.5f32, 2f32, 1e3);"),
            expected(&[
                ("1.5 → 2.5", "const F: (f64, f32, f32, f64) = (2.5, 2.5f32, 2f32, 1e3);"),
                ("2.5f32 → 3.5f32", "const F: (f64, f32, f32, f64) = (1.5, 3.5f32, 2f32, 1e3);"),
                // Rust reads `2f32` as an integer literal with a float's suffix.
                ("2f32 → 3f32", "const F: (f64, f32, f32, f64) = (1.5, 2.5f32, 3f32, 1e3);"),
                ("1e3 → 1001.0", "const F: (f64, f32, f32, f64) = (1.5, 2.5f32, 2f32, 1001.0);"),
            ])
        );
    }

    #[test]
    fn a_string_gains_an_x_before_its_closing_quote_whatever_its_kind() {
        let source = r##"const S: (&str, &[u8], &str, &str) = ("shop", b"ab", r#"say "hi""#, "tab\t");"##;
        let descriptions: Vec<String> =
            mutated(source).into_iter().map(|(description, _)| description).collect();
        assert_eq!(
            descriptions,
            [
                r#""shop" → "shopx""#,
                r#"b"ab" → b"abx""#,
                r##"r#"say "hi""# → r#"say "hi"x"#"##,
                r#""tab\t" → "tab\tx""#,
            ]
        );
    }

    #[test]
    fn a_character_and_a_byte_are_left_alone_and_a_boolean_flips() {
        assert_eq!(
            mutated("const C: (char, bool, u8) = ('a', true, b'z');"),
            expected(&[("true → false", "const C: (char, bool, u8) = ('a', false, b'z');")])
        );
    }

    #[test]
    fn a_slice_loses_its_last_item_before_its_items_change() {
        assert_eq!(
            mutated(r#"const ENV: &[&str] = &["PATH", "LOG_LEVEL"];"#),
            expected(&[
                (r#""LOG_LEVEL" removed"#, r#"const ENV: &[&str] = &["PATH"];"#),
                (r#""PATH" → "PATHx""#, r#"const ENV: &[&str] = &["PATHx", "LOG_LEVEL"];"#),
                (r#""LOG_LEVEL" → "LOG_LEVELx""#, r#"const ENV: &[&str] = &["PATH", "LOG_LEVELx"];"#),
            ])
        );
    }

    #[test]
    fn a_last_item_goes_with_the_comma_before_it_and_an_only_one_with_its_trailing_comma() {
        let removed = |source: &str| mutated(source).into_iter().next().unwrap();
        assert_eq!(
            removed("const L: &[u8] = &[1, 2,];"),
            ("2 removed".into(), "const L: &[u8] = &[1,];".into())
        );
        assert_eq!(
            removed("const L: &[u8] = &[ 7, ];"),
            ("7 removed".into(), "const L: &[u8] = &[];".into())
        );
    }

    #[test]
    fn a_vec_loses_its_last_item_as_a_slice_does() {
        assert_eq!(
            mutated("const V: Vec<u8> = vec![1, 2];"),
            expected(&[
                ("2 removed", "const V: Vec<u8> = vec![1];"),
                ("1 → 2", "const V: Vec<u8> = vec![2, 2];"),
                ("2 → 3", "const V: Vec<u8> = vec![1, 3];"),
            ])
        );
        assert_eq!(
            mutated("const V: Vec<u8> = vec![5];")[0],
            ("5 removed".into(), "const V: Vec<u8> = vec![];".into())
        );
    }

    #[test]
    fn an_array_whose_length_its_type_writes_loses_no_item() {
        let descriptions = |source: &str| mutated(source).into_iter().map(|(d, _)| d).collect::<Vec<_>>();
        assert_eq!(descriptions(r#"const K: [&str; 2] = ["a", "b"];"#), [r#""a" → "ax""#, r#""b" → "bx""#]);
        assert_eq!(descriptions("const R: &[u8; 2] = &[1, 2];"), ["1 → 2", "2 → 3"]);
        assert_eq!(
            descriptions("const N: &[[u8; 2]] = &[[1, 2], [3, 4]];"),
            ["[3, 4] removed", "1 → 2", "2 → 3", "3 → 4", "4 → 5"]
        );
    }

    #[test]
    fn the_fields_of_tuples_and_structs_are_values_at_any_depth() {
        let source =
            r#"const C: Config = Config { port: 80, names: &["a"], retry: (3, Duration::from_secs(5)) };"#;
        let descriptions: Vec<String> =
            mutated(source).into_iter().map(|(description, _)| description).collect();
        assert_eq!(descriptions, ["80 → 81", r#""a" removed"#, r#""a" → "ax""#, "3 → 4", "5 → 6"]);
    }

    #[test]
    fn what_runs_when_called_types_and_lengths_are_left_alone() {
        let descriptions = |source: &str| mutated(source).into_iter().map(|(d, _)| d).collect::<Vec<_>>();
        assert!(descriptions("static S: LazyLock<u8> = LazyLock::new(|| 5);").is_empty());
        assert_eq!(descriptions("const X: u8 = { fn one() -> u8 { 1 } one() + 2 };"), ["2 → 3"]);
        assert_eq!(descriptions("const A: [u8; 3] = [0; 3];"), ["0 → 1"]);
        assert_eq!(descriptions("const Z: u32 = parse::<4>(7) as u32;"), ["7 → 8"]);
    }

    #[test]
    fn an_edit_is_placed_in_bytes_after_a_wider_character_on_its_line() {
        assert_eq!(
            mutated(r#"const SHOP: (&str, u16) = ("café", 8080);"#)[1],
            ("8080 → 8081".into(), r#"const SHOP: (&str, u16) = ("café", 8081);"#.into())
        );
    }
}
