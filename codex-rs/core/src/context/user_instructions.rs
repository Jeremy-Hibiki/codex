use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UserInstructions {
    pub(crate) directory: Option<String>,
    pub(crate) text: String,
}

impl ContextualUserFragment for UserInstructions {
    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("# GREVO.md instructions", "</INSTRUCTIONS>")
    }

    fn matches_text(text: &str) -> bool {
        let trimmed = text.trim_start();
        let starts_with_supported_marker = [Self::type_markers().0, "# AGENTS.md instructions"]
            .iter()
            .any(|marker| {
                trimmed
                    .get(..marker.len())
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(marker))
            });
        let trimmed = trimmed.trim_end();
        let end_marker = Self::type_markers().1;
        let ends_with_marker = trimmed
            .get(trimmed.len().saturating_sub(end_marker.len())..)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(end_marker));
        starts_with_supported_marker && ends_with_marker
    }

    fn body(&self) -> String {
        let directory = self
            .directory
            .as_ref()
            .map(|directory| format!(" for {directory}"))
            .unwrap_or_default();
        format!("{directory}\n\n<INSTRUCTIONS>\n{}\n", self.text)
    }
}
