//! `undocumented`: by surface, the symbols that are not meant to be documented, which
//! `uncovered` and the report's symbol counts leave out of what is left to attest.

use super::{Fields, ProjectConfig, SurfaceEntry};

/// Why a glob of symbol names cannot start with `!`.
const NAME_NEGATION: &str = "a glob starting with \"!\" matches every symbol but the ones it names; list the symbols that are not meant to be documented";

impl ProjectConfig {
    /// The globs of the symbols of `surface` that are not meant to be documented; none
    /// when the config names none.
    pub fn undocumented_globs(&self, surface: &str) -> &[String] {
        let named = self.undocumented.iter().find(|(name, _)| name == surface);
        named.map_or(&[], |(_, globs)| globs.as_slice())
    }
}

/// The globs the config gives each of its `surfaces`, in its order; none when it gives none.
pub(super) fn read_undocumented(
    fields: &mut Fields,
    surfaces: &[(String, SurfaceEntry)],
) -> Vec<(String, Vec<String>)> {
    fields
        .nested("undocumented", |entries| {
            let names: Vec<String> = entries.object.keys().cloned().collect();
            names.into_iter().filter_map(|name| surface_globs(entries, surfaces, name)).collect()
        })
        .unwrap_or_default()
}

fn surface_globs(
    entries: &mut Fields,
    surfaces: &[(String, SurfaceEntry)],
    name: String,
) -> Option<(String, Vec<String>)> {
    if !surfaces.iter().any(|(surface, _)| *surface == name) {
        let configured: Vec<&str> = surfaces.iter().map(|(surface, _)| surface.as_str()).collect();
        let message = format!(
            "\"undocumented\" has {name:?}, which is not a surface (configured: {})",
            configured.join(", ")
        );
        entries.problems.push(message);
        return None;
    }
    let globs = entries.globs(&name, entries.object.get(&name), NAME_NEGATION);
    Some((name, globs))
}

#[cfg(test)]
mod tests {
    use crate::config::parse_config;
    use serde_json::json;

    fn undocumented(value: serde_json::Value) -> Result<Vec<(String, Vec<String>)>, String> {
        let surfaces = json!({ "default": "x", "server": "y" });
        let config = json!({ "pages": [], "surfaces": surfaces, "adapter": "x", "undocumented": value });
        parse_config(&config.to_string()).map(|c| c.undocumented).map_err(|e| e.to_string())
    }

    #[test]
    fn a_surface_names_the_globs_of_its_symbols_that_are_not_meant_to_be_documented() {
        let read = undocumented(json!({ "server": ["Program*"], "default": ["internal*", "Debug"] }));
        let globs = |globs: &[&str]| globs.iter().map(|g| g.to_string()).collect::<Vec<_>>();
        assert_eq!(
            read.unwrap(),
            vec![("server".into(), globs(&["Program*"])), ("default".into(), globs(&["internal*", "Debug"]))]
        );
        assert_eq!(undocumented(json!({})).unwrap(), vec![]);
        let config = parse_config(
            r#"{"pages":[],"surfaces":{"default":"x"},"adapter":"x","undocumented":{"default":["a*"]}}"#,
        )
        .unwrap();
        assert_eq!(config.undocumented_globs("default"), ["a*"]);
        assert!(config.undocumented_globs("other").is_empty());
    }

    #[test]
    fn a_name_that_is_no_surface_or_globs_that_cannot_be_read_are_refused() {
        for (bad, said) in [
            (json!(["internal*"]), "\"undocumented\" must be an object"),
            (
                json!({ "client": ["internal*"] }),
                "\"undocumented\" has \"client\", which is not a surface (configured: default, server)",
            ),
            (json!({ "default": "internal*" }), "\"undocumented.default\" must be an array of globs"),
            (json!({ "default": [""] }), "\"undocumented.default\" has a glob that cannot be read"),
            (
                json!({ "default": ["!Program"] }),
                "\"undocumented.default\" has \"!Program\": a glob starting with \"!\" matches every symbol but the ones it names",
            ),
        ] {
            let error = undocumented(bad.clone()).unwrap_err();
            assert!(error.contains(said), "{bad}: {error}");
        }
    }
}
