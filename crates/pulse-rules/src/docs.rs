//! Rule documentation (`docs/rules/<ID>.md`), embedded at build time.

/// Generates [`docs`] as a match over the listed IDs.
macro_rules! rule_docs {
    ($($id:literal),+ $(,)?) => {
        /// The Markdown documentation of the built-in rule `id`.
        pub fn docs(id: &str) -> Option<&'static str> {
            match id {
                $($id => Some(include_str!(concat!("../../../docs/rules/", $id, ".md"))),)+
                _ => None,
            }
        }
    };
}

rule_docs!(
    "AI-COG-001",
    "AI-COG-002",
    "AI-COG-003",
    "AI-COG-004",
    "AI-COG-005",
    "AI-COG-006",
    "AI-COG-007",
    "AI-EFF-001",
    "AI-EFF-002",
    "AI-EFF-003",
    "AI-EFF-004",
    "AI-EFF-005",
    "AI-EFF-006",
    "AI-EFF-007",
    "AI-SEC-001",
    "AI-SEC-002",
    "AI-SEC-003",
    "AI-SEC-004",
    "AI-SEC-005",
    "AI-SEC-006",
    "AI-SEC-007",
    "AI-SEC-008",
    "AI-SEC-009",
    "AI-SEC-010",
    "AI-SEC-011",
    "AI-SEC-012",
    "AI-SEC-013",
    "AI-STR-001",
    "AI-STR-002",
    "AI-STR-003",
    "AI-STR-004",
    "AI-STR-005",
    "AI-STR-006",
    "AI-STR-007",
    "AI-STR-008",
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_rule_has_docs_with_its_id_as_title() {
        for m in crate::all_meta() {
            let doc = docs(m.id).unwrap_or_else(|| panic!("no docs for {}", m.id));
            assert!(doc.starts_with(&format!("# {} — ", m.id)), "{}", m.id);
        }
        assert_eq!(docs("AI-SEC-999"), None);
    }
}
