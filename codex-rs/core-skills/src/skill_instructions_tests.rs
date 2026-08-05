use super::*;

fn plaintext_injection(name: &str) -> SkillInjection {
    SkillInjection {
        name: name.to_string(),
        path: format!("/tmp/{name}/SKILL.md"),
        contents: "# Real instructions\nrun scripts/build.sh".to_string(),
        encrypted: false,
        token: None,
    }
}

fn encrypted_injection(name: &str, token: &str) -> SkillInjection {
    SkillInjection {
        encrypted: true,
        token: Some(token.to_string()),
        ..plaintext_injection(name)
    }
}

#[test]
fn plaintext_body_keeps_existing_format() {
    let skill = plaintext_injection("demo");
    let instructions = SkillInstructions::from(&skill);
    assert_eq!(
        instructions.body(),
        "\n<name>demo</name>\n<path>/tmp/demo/SKILL.md</path>\n# Real instructions\nrun scripts/build.sh\n"
    );
}

#[test]
fn encrypted_body_returns_only_the_sentinel_token() {
    let skill = encrypted_injection(
        "secret-skill",
        "[SENSITIVE_SKILL_TOKEN:thread-1:a1b2c3d4e5f60718293a4b5c6d7e8f90]",
    );
    let instructions = SkillInstructions::from(&skill);
    assert_eq!(
        instructions.body(),
        "[SENSITIVE_SKILL_TOKEN:thread-1:a1b2c3d4e5f60718293a4b5c6d7e8f90]"
    );
    assert!(!instructions.body().contains("# Real instructions"));
}

#[test]
fn encrypted_without_token_falls_back_to_original_format() {
    let skill = encrypted_injection("secret-skill", "");
    let mut without_token = skill;
    without_token.token = None;
    let instructions = SkillInstructions::from(&without_token);
    assert!(instructions.body().contains("<name>secret-skill</name>"));
    assert!(instructions.body().contains("# Real instructions"));
}
