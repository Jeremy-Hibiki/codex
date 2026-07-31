use codex_context_fragments::ContextualUserFragment;

use crate::injection::SkillInjection;

#[derive(Debug, Clone, PartialEq)]
pub struct SkillInstructions {
    name: String,
    path: String,
    contents: String,
    encrypted: bool,
    token: Option<String>,
}

impl From<&SkillInjection> for SkillInstructions {
    fn from(skill: &SkillInjection) -> Self {
        Self {
            name: skill.name.clone(),
            path: skill.path.clone(),
            contents: skill.contents.clone(),
            encrypted: skill.encrypted,
            token: skill.token.clone(),
        }
    }
}

impl ContextualUserFragment for SkillInstructions {
    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<skill>", "</skill>")
    }

    fn body(&self) -> String {
        if self.encrypted
            && let Some(token) = &self.token
        {
            return token.clone();
        }
        format!(
            "\n<name>{}</name>\n<path>{}</path>\n{}\n",
            self.name, self.path, self.contents
        )
    }
}

#[cfg(test)]
#[path = "skill_instructions_tests.rs"]
mod tests;
