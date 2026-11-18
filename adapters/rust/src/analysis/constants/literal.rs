//! Whether a constant's type and value can be written into another program and mean the
//! same there: they name nothing of their crate but other such constants of their
//! module.
//!
//! A type is a primitive type (`u32`, `f64`, `bool`, `char`, `str`), `Duration`, or
//! references, slices, arrays and tuples of those. A value is made of literals, operators,
//! casts to such types, arrays, tuples, references, fields and indexes, other constants of
//! the module by their bare names, and the constants, functions and methods of std (`u32::MAX`,
//! `Duration::from_secs(30)`, `"a".len()`). The compiler then computes the value in the
//! fact's program as it does in the crate, in a const context, so nothing runs and the value
//! has the type it has in the crate.

use quote::ToTokens;
use syn::{Expr, Path, Type};

/// The primitive types, which every program names alike.
const PRIMITIVES: [&str; 17] = [
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64",
    "bool", "char", "str",
];

/// The crates whose paths mean the same in every program.
const STANDARD: [&str; 2] = ["std", "core"];

/// The path of `Duration` in std, as a `use` declaration of a module names it.
pub(crate) const DURATION: [&str; 3] = ["std", "time", "Duration"];

/// What a module calls std's types: the names its `use` declarations give `Duration`.
pub(crate) struct Names<'a> {
    pub(crate) duration: &'a [String],
}

impl Names<'_> {
    /// Checks a constant's type, and adds the bare names its array lengths read.
    pub(crate) fn check_type(&self, ty: &Type, reads: &mut Vec<String>) -> Result<(), String> {
        let parts = match ty {
            Type::Path(path) if path.qself.is_none() && self.names_a_type(&path.path) => Vec::new(),
            Type::Array(array) => {
                self.check_value(&array.len, reads)?;
                vec![&*array.elem]
            }
            other => type_parts(other).ok_or_else(|| {
                format!("its type `{}` is not made of primitive types and Duration", shown(other))
            })?,
        };
        parts.into_iter().try_for_each(|part| self.check_type(part, reads))
    }

    /// Checks a constant's value, and adds the bare names it reads, which must be other
    /// constants of its module.
    pub(crate) fn check_value(&self, expr: &Expr, reads: &mut Vec<String>) -> Result<(), String> {
        let parts = match expr {
            Expr::Lit(lit) if !matches!(lit.lit, syn::Lit::Verbatim(_)) => Vec::new(),
            Expr::Path(path) if path.qself.is_none() => return self.check_path(&path.path, reads),
            Expr::Cast(cast) => {
                self.check_type(&cast.ty, reads)?;
                vec![&*cast.expr]
            }
            Expr::Call(call) => self.call_arguments(call)?,
            other => operands(other)
                .or_else(|| elements(other))
                .ok_or_else(|| format!("its value `{}` is not written with literals", shown(other)))?,
        };
        parts.into_iter().try_for_each(|part| self.check_value(part, reads))
    }

    /// The arguments of a call of a function of std.
    fn call_arguments<'e>(&self, call: &'e syn::ExprCall) -> Result<Vec<&'e Expr>, String> {
        match &*call.func {
            Expr::Path(callee) if callee.qself.is_none() && self.of_std(&callee.path) => {
                Ok(call.args.iter().collect())
            }
            callee => Err(format!("its value calls `{}`", shown(callee))),
        }
    }

    /// Checks a path a value names: a constant of the module, or something of std.
    fn check_path(&self, path: &Path, reads: &mut Vec<String>) -> Result<(), String> {
        if let Some(name) = path.get_ident() {
            reads.push(name.to_string());
            return Ok(());
        }
        if self.of_std(path) {
            return Ok(());
        }
        Err(format!("its value names `{}`, which is not a constant of its module", shown(path)))
    }

    /// Whether a type path names a primitive type or `Duration`.
    fn names_a_type(&self, path: &Path) -> bool {
        let written = segments(path);
        let plain = path.segments.iter().all(|segment| segment.arguments.is_none());
        let named = match written.as_slice() {
            [one] => PRIMITIVES.contains(&one.as_str()) || self.duration.contains(one),
            [root, rest @ ..] => STANDARD.contains(&root.as_str()) && rest == &DURATION[1..],
            [] => false,
        };
        plain && named
    }

    /// Whether a value's path is something of std: rooted at std or core, or an item of a
    /// primitive type or of `Duration` (`u32::MAX`, `Duration::from_secs`).
    fn of_std(&self, path: &Path) -> bool {
        let written = segments(path);
        let plain = path.segments.iter().all(|segment| segment.arguments.is_none());
        let rooted = match written.as_slice() {
            [root, _, ..] if STANDARD.contains(&root.as_str()) => true,
            [owner, _] => PRIMITIVES.contains(&owner.as_str()) || self.duration.contains(owner),
            _ => false,
        };
        plain && rooted
    }
}

/// The types a reference, a slice, a tuple or parentheses are made of.
fn type_parts(ty: &Type) -> Option<Vec<&Type>> {
    Some(match ty {
        Type::Reference(reference) if reference.mutability.is_none() => vec![&*reference.elem],
        Type::Slice(slice) => vec![&*slice.elem],
        Type::Tuple(tuple) => tuple.elems.iter().collect(),
        Type::Paren(paren) => vec![&*paren.elem],
        Type::Group(group) => vec![&*group.elem],
        _ => return None,
    })
}

/// The operands of an operator a constant's value may use.
fn operands(expr: &Expr) -> Option<Vec<&Expr>> {
    Some(match expr {
        Expr::Unary(unary) if !matches!(unary.op, syn::UnOp::Deref(_)) => vec![&*unary.expr],
        Expr::Binary(binary) => vec![&*binary.left, &*binary.right],
        Expr::Paren(paren) => vec![&*paren.expr],
        Expr::Group(group) => vec![&*group.expr],
        Expr::Reference(reference) if reference.mutability.is_none() => vec![&*reference.expr],
        _ => return None,
    })
}

/// The parts of an array, a tuple, a field or an index, or a method call (which only std's
/// methods can be in a const context).
fn elements(expr: &Expr) -> Option<Vec<&Expr>> {
    Some(match expr {
        Expr::Array(array) => array.elems.iter().collect(),
        Expr::Repeat(repeat) => vec![&*repeat.expr, &*repeat.len],
        Expr::Tuple(tuple) => tuple.elems.iter().collect(),
        Expr::Field(field) => vec![&*field.base],
        Expr::Index(index) => vec![&*index.expr, &*index.index],
        Expr::MethodCall(call) if call.turbofish.is_none() => {
            std::iter::once(&*call.receiver).chain(&call.args).collect()
        }
        _ => return None,
    })
}

/// The names of a path's segments.
fn segments(path: &Path) -> Vec<String> {
    path.segments.iter().map(|segment| segment.ident.to_string()).collect()
}

/// Code as a message quotes it.
fn shown(code: &impl ToTokens) -> String {
    code.to_token_stream().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked(declaration: &str) -> Result<Vec<String>, String> {
        let item: syn::ItemConst = syn::parse_str(declaration).unwrap();
        let duration = ["Duration".to_string()];
        let names = Names { duration: &duration };
        let mut reads = Vec::new();
        names.check_type(&item.ty, &mut reads)?;
        names.check_value(&item.expr, &mut reads)?;
        Ok(reads)
    }

    #[test]
    fn literal_values_of_std_types_are_reproduced() {
        let read = [
            "const A: u16 = 8080;",
            "const B: f64 = 1.0 / 3.0;",
            "const C: &str = \"nl\";",
            "const D: [(&str, u32); 2] = [(\"NL\", 1), (\"DE\", 2)];",
            "const E: &[char] = &['a', 'b'];",
            "const F: u64 = u32::MAX as u64 + 1;",
            "const G: Duration = Duration::from_secs(30);",
            "const H: std::time::Duration = core::time::Duration::from_millis(5);",
            "const I: usize = \"abc\".len();",
            "const J: [u8; 4] = [0; 4];",
            "const L: [u8; 2] = [0; 2usize.pow(1)];",
            "const K: bool = !(1 > 2) && true;",
        ];
        for declaration in read {
            assert_eq!(checked(declaration), Ok(Vec::new()), "{declaration}");
        }
        assert_eq!(checked("const T: u64 = BASE * 2 + OTHER;"), Ok(vec!["BASE".into(), "OTHER".into()]));
        assert_eq!(checked("const U: [u8; N] = [0; N];"), Ok(vec!["N".into(), "N".into()]));
    }

    #[test]
    fn what_names_the_crate_is_not_reproduced() {
        let refused = [
            ("const A: Level = Level(2);", "its type `Level` is not made of primitive types and Duration"),
            ("const B: u16 = port();", "its value calls `port`"),
            (
                "const C: u16 = config::PORT;",
                "its value names `config :: PORT`, which is not a constant of its module",
            ),
            ("const D: u32 = { 1 + 1 };", "its value `{ 1 + 1 }` is not written with literals"),
            ("const E: &str = env!(\"X\");", "its value `env ! (\"X\")` is not written with literals"),
            (
                "const F: Vec<u8> = Vec::new();",
                "its type `Vec < u8 >` is not made of primitive types and Duration",
            ),
        ];
        for (declaration, why) in refused {
            assert_eq!(checked(declaration), Err(why.to_string()), "{declaration}");
        }
    }
}
