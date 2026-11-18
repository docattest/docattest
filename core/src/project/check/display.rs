//! The values of data blocks, displayed by the adapter. A `field=` fact compares a value that
//! a JSON, TOML or YAML block writes in its format's spelling (`true`, `null`, `1.0`) with a
//! value the adapter displays in its language's, which may be another. The core knows no
//! language's, so it asks the adapter to display the block's values as it displays a fact's
//! (PROTOCOL.md §3, `display`).

use std::collections::HashMap;

use serde_json::{Value, json};

use crate::adapter::{Adapter, Pending};
use crate::api::attestation::{Attestation, Body, Fact, FactTarget};
use crate::api::errors::{Error, Result};

/// The capability, and the method, by which an adapter displays values.
const DISPLAY: &str = "display";

/// Asks the adapter to display every scalar that the targets of `facts` read from a data
/// block, in order. Nothing is asked when none does, or of an adapter that does not list
/// `display`; its fields are compared as JSON writes them.
pub(super) fn send_display(adapter: &Adapter, facts: &[&Attestation]) -> Option<Pending> {
    let values: Vec<&Value> = facts.iter().flat_map(|fact| field_scalars(fact)).collect();
    (adapter.can(DISPLAY) && !values.is_empty()).then(|| adapter.send(DISPLAY, json!({ "values": values })))
}

/// The targets of the facts whose data block values the adapter displayed, by fact id.
pub(super) struct Displayed {
    targets: HashMap<String, FactTarget>,
}

impl Displayed {
    /// The answer to [`send_display`] for the same `facts`, which holds a text for each value
    /// sent, in order: each field target spelled with its own.
    pub(super) fn wait(pending: Option<Pending>, facts: &[&Attestation]) -> Result<Displayed> {
        let Some(pending) = pending else { return Ok(Displayed { targets: HashMap::new() }) };
        let answer = pending.wait()?;
        let sent = facts.iter().map(|fact| field_scalars(fact).len()).sum::<usize>();
        let texts: Option<Vec<String>> = answer
            .as_array()
            .and_then(|items| items.iter().map(|t| t.as_str().map(str::to_string)).collect());
        let Some(texts) = texts.filter(|texts| texts.len() == sent) else {
            return Err(Error::internal(format!(
                "display did not answer one string for each of {sent} values"
            )));
        };
        let mut texts = texts.into_iter();
        let mut targets = HashMap::new();
        for fact in facts {
            if let Body::Fact(Fact { target: Ok(target @ FactTarget::Field { .. }), .. }) = &fact.body {
                let spelled = target.spelled(|_| texts.next().expect("a text for each value sent"));
                targets.insert(fact.id.clone(), spelled);
            }
        }
        Ok(Displayed { targets })
    }

    /// The target `fact` is compared with: its field's values as the adapter displayed them;
    /// any other target, or any target when nothing was displayed, as it is.
    pub(super) fn target<'a>(&'a self, fact: &Attestation, target: &'a FactTarget) -> &'a FactTarget {
        self.targets.get(&fact.id).unwrap_or(target)
    }
}

/// The scalars a fact's target reads from a data block; none for any other target.
fn field_scalars(fact: &Attestation) -> &[Value] {
    match &fact.body {
        Body::Fact(Fact { target: Ok(FactTarget::Field { value, .. }), .. }) => value.scalars(),
        _ => &[],
    }
}
