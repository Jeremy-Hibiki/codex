use super::*;
use crate::cache::CachedContent;

fn cached(cache: &mut ContentCache, session: &str, plaintext: &str) -> String {
    let hex = cache.store(
        session,
        CachedContent {
            plaintext: plaintext.to_string(),
            skill_name: "skill".to_string(),
            base_dir: None,
        },
    );
    format!("[SENSITIVE_SKILL_TOKEN:{session}:{hex}]")
}

#[test]
fn rehydrates_same_session_token() {
    let mut cache = ContentCache::new(64);
    let token = cached(&mut cache, "t1", "# Real skill content");
    let out = rehydrate_text(&format!("before {token} after"), Some("t1"), &cache);
    assert_eq!(out, "before # Real skill content after");
}

#[test]
fn refuses_cross_session_token() {
    let mut cache = ContentCache::new(64);
    let token = cached(&mut cache, "t1", "secret");
    let out = rehydrate_text(&token, Some("t2"), &cache);
    assert_eq!(out, token);
}

#[test]
fn keeps_stale_token_when_content_missing() {
    let mut cache = ContentCache::new(64);
    let token = cached(&mut cache, "t1", "secret");
    cache.drop_session("t1");
    let out = rehydrate_text(&token, Some("t1"), &cache);
    assert_eq!(out, token);
}

#[test]
fn leaves_invalid_token_text_untouched() {
    let cache = ContentCache::new(64);
    let text = "[SENSITIVE_SKILL_TOKEN:t1:nothex]";
    assert_eq!(rehydrate_text(text, Some("t1"), &cache), text);
}

#[test]
fn rehydrates_multiple_tokens() {
    let mut cache = ContentCache::new(64);
    let a = cached(&mut cache, "t1", "alpha");
    let b = cached(&mut cache, "t1", "beta");
    let out = rehydrate_text(&format!("{a}|{b}"), Some("t1"), &cache);
    assert_eq!(out, "alpha|beta");
}

#[test]
fn framing_includes_skill_name_and_original_base_dir() {
    let framed = wrap_with_framing(
        "# doc",
        "secret-skill",
        Some("~/.codex/skills/secret-skill"),
    );
    assert!(framed.contains("<skill_name>secret-skill</skill_name>"));
    assert!(framed.contains("<base_directory>~/.codex/skills/secret-skill</base_directory>"));
    assert!(framed.contains("# doc"));
}

#[test]
fn framing_without_base_dir_omits_anchor() {
    let framed = wrap_with_framing("# doc", "secret-skill", None);
    assert!(!framed.contains("base_directory"));
}

#[test]
fn framed_content_never_contains_mem_root_path() {
    let framed = wrap_with_framing("# doc", "s", Some("~/.codex/skills/s"));
    assert!(!framed.contains("/dev/shm/fm-agent-security"));
}

#[test]
fn rehydration_never_injects_mem_root_path() {
    let mut cache = ContentCache::new(64);
    let token = cached(&mut cache, "t1", "run scripts/build.sh");
    let out = rehydrate_text(&token, Some("t1"), &cache);
    assert!(!out.contains("/dev/shm/fm-agent-security"));
}
