//! Manifests: the project files a fact reads as data with `manifest("<path>")`, such as a
//! package's manifest or a config file, parsed by their format. The core reads formats
//! (JSON, TOML, YAML, `go.mod`); what a field of a manifest means is the page's to say.
//! Which files facts read, and when, is `src/fact_files/`'s.

pub mod format;
mod go_mod;
mod yaml;
