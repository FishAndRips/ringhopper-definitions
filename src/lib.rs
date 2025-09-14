//! General-use definitions parsing library.
//!
//! This can be used for writing parsers for tags.
#![no_std]
#![deny(missing_docs)]

extern crate alloc;
extern crate serde_json;

mod types;

use spin::lazy::Lazy;
pub use types::*;

/// Load all built-in definitions.
static DEFINITIONS: Lazy<ParsedDefinitions> = Lazy::new(|| {
    let values = get_all_definitions();
    let mut parsed = ParsedDefinitions::default();
    parsed.load_from_json(&values);
    parsed.finalize_and_assert_valid();
    parsed.resolve_parent_class_references();

    parsed
});

/// Load all built-in definitions.
pub fn load_all_definitions() -> &'static ParsedDefinitions {
    &*DEFINITIONS
}

#[cfg(test)]
mod test {
    use crate::load_all_definitions;

    #[test]
    fn loading_all_definitions_succeeds() {
        load_all_definitions();
    }
}
