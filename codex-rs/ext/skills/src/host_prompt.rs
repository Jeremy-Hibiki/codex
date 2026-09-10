use std::collections::HashSet;

use codex_extension_api::ContextualUserFragment;
use codex_skills::SkillMetadata;
use codex_skills::normalize_skill_path;
use fm_encrypted_skills::runtime::EncryptedSkillRuntime;

use crate::HostSkillsSnapshot;
use crate::fragments::SkillInstructions;
use crate::render::truncate_main_prompt_contents;

/// Host skill prompts already supplied or superseded by an extension.
///
/// Core preserves its host skill invocation lifecycle while avoiding duplicate
/// prompts and retaining executor/orchestrator skill precedence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InjectedHostSkillPrompts {
    paths: HashSet<String>,
    superseded_paths: HashSet<String>,
}

impl InjectedHostSkillPrompts {
    pub fn insert_path(&mut self, path: impl Into<String>) {
        let path = path.into();
        self.paths.insert(normalize_host_skill_path(&path));
        self.paths.insert(path);
    }

    pub fn insert_superseded_path(&mut self, path: impl Into<String>) {
        let path = path.into();
        self.superseded_paths
            .insert(normalize_host_skill_path(&path));
        self.insert_path(path);
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn contains_path(&self, path: &str) -> bool {
        self.paths.contains(path) || self.paths.contains(&normalize_host_skill_path(path))
    }

    pub fn is_superseded_path(&self, path: &str) -> bool {
        self.superseded_paths
            .contains(&normalize_host_skill_path(path))
    }
}

/// Prompt fragments and read outcomes for a set of selected host skills.
pub struct HostSkillPrompts {
    pub fragments: Vec<Box<dyn ContextualUserFragment + Send>>,
    pub injected: Vec<SkillMetadata>,
    pub warnings: Vec<String>,
}

fn normalize_host_skill_path(path: &str) -> String {
    normalize_skill_path(path).replace('\\', "/")
}

impl HostSkillsSnapshot {
    /// Reads selected host skills and builds their model-visible prompt fragments.
    ///
    /// Core calls this directly, including for hosts without an installed skills extension.
    #[tracing::instrument(
        level = "trace",
        skip_all,
        fields(selected_skill_count = selected_skills.len())
    )]
    pub async fn load_skill_prompts(
        &self,
        selected_skills: &[SkillMetadata],
        encrypted_skills: Option<&EncryptedSkillRuntime>,
        session_id: &str,
    ) -> HostSkillPrompts {
        let mut prompts = HostSkillPrompts {
            fragments: Vec::with_capacity(selected_skills.len()),
            injected: Vec::with_capacity(selected_skills.len()),
            warnings: Vec::new(),
        };

        for skill in selected_skills {
            let loaded: Result<String, String> = if skill.is_encrypted() {
                // An encrypted skill's SKILL.md is a stub on disk. Decrypt the
                // package now and inject the sentinel token instead of the
                // plaintext; the token is rehydrated at request build time.
                match encrypted_skills {
                    Some(runtime) => encrypted_skill_token(runtime, session_id, skill),
                    None => Err(
                        "encrypted skills require an envelope SDK runtime to be configured"
                            .to_string(),
                    ),
                }
            } else {
                self.read_skill_text(skill)
                    .await
                    .map_err(|err| format!("{err:#}"))
            };
            match loaded {
                Ok(contents) => {
                    // The sentinel token is already length-bounded; plugin
                    // truncation applies to real skill bodies only.
                    let (contents, truncated) =
                        if !skill.is_encrypted() && self.outcome().is_agent_plugin_skill(skill) {
                            truncate_main_prompt_contents(&contents)
                        } else {
                            (contents, false)
                        };
                    if truncated {
                        prompts.warnings.push(format!(
                            "Skill `{}` exceeded the main prompt context limit and was truncated.",
                            skill.name
                        ));
                    }
                    prompts.fragments.push(Box::new(SkillInstructions {
                        name: skill.name.clone(),
                        path: skill.path_to_skills_md.to_string_lossy().into_owned(),
                        contents,
                        resource_access: None,
                    }));
                    prompts.injected.push(skill.clone());
                }
                Err(err) => {
                    prompts.warnings.push(format!(
                        "Failed to load skill {} at {}: {err}",
                        skill.name,
                        skill.path_to_skills_md.display()
                    ));
                }
            }
        }

        prompts
    }
}

/// Registers (and decrypts) an encrypted skill, returning the sentinel token
/// that stands in for its body in the model-visible prompt.
fn encrypted_skill_token(
    runtime: &EncryptedSkillRuntime,
    session_id: &str,
    skill: &SkillMetadata,
) -> Result<String, String> {
    let skill_dir = skill
        .path_to_skills_md
        .parent()
        .ok_or_else(|| "skill path has no parent directory".to_string())?;
    let default_package = format!("{}.zip.enc", skill.name);
    let package_name = skill
        .encryption
        .as_ref()
        .and_then(|encryption| encryption.package.as_deref())
        .filter(|package| !package.is_empty())
        .unwrap_or(&default_package);
    let package_path = skill_dir.join(package_name);
    runtime
        .load_or_register(session_id, &skill.name, package_path.as_path())
        .map_err(|err| format!("{err:#}"))
}
