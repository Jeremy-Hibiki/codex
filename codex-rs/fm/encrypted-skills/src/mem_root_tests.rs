use std::path::PathBuf;

use super::*;

fn entry(rel_path: &str, contents: &str) -> PackageEntry {
    PackageEntry {
        rel_path: PathBuf::from(rel_path),
        contents: contents.as_bytes().to_vec(),
    }
}

#[test]
fn writes_package_entries_preserving_layout() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("fm_skill_security_abc");
    let entries = vec![
        entry("SKILL.md", "# Real skill\n"),
        entry("scripts/build.sh", "#!/bin/sh\necho hi\n"),
        entry("templates/viewer.html", "<html></html>"),
    ];
    write_package_entries(&entries, &target).unwrap();

    assert_eq!(
        fs::read_to_string(target.join("SKILL.md")).unwrap(),
        "# Real skill\n"
    );
    assert_eq!(
        fs::read_to_string(target.join("scripts/build.sh")).unwrap(),
        "#!/bin/sh\necho hi\n"
    );
    assert_eq!(
        fs::read_to_string(target.join("templates/viewer.html")).unwrap(),
        "<html></html>"
    );
}

#[test]
fn rejects_zip_slip_parent_dir_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("out");
    let entries = vec![entry("../evil.txt", "x")];
    assert!(matches!(
        write_package_entries(&entries, &target),
        Err(EnvelopeError::InvalidEntry { .. })
    ));
    assert!(!tmp.path().join("evil.txt").exists());
}

#[test]
fn rejects_absolute_entry_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("out");
    let entries = vec![entry("/etc/passwd", "x")];
    assert!(matches!(
        write_package_entries(&entries, &target),
        Err(EnvelopeError::InvalidEntry { .. })
    ));
}

#[test]
fn init_mem_root_removes_stale_decrypted_dirs_only() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("mem-root");
    fs::create_dir_all(root.join("fm_skill_security_stale")).unwrap();
    fs::create_dir_all(root.join("other")).unwrap();
    fs::write(root.join("keep.txt"), "keep").unwrap();

    init_mem_root(&root).unwrap();

    assert!(!root.join("fm_skill_security_stale").exists());
    assert!(root.join("other").exists());
    assert!(root.join("keep.txt").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn init_mem_root_removes_dead_process_namespaces_and_keeps_live() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("mem-root");
    fs::create_dir_all(root.join("p99999999")).unwrap();
    fs::create_dir_all(root.join(process_namespace())).unwrap();
    fs::write(root.join("keep.txt"), "keep").unwrap();

    init_mem_root(&root).unwrap();

    assert!(!root.join("p99999999").exists());
    assert!(root.join(process_namespace()).exists());
    assert!(root.join("keep.txt").exists());
}

#[test]
fn init_mem_root_once_initializes_only_the_first_root() {
    let tmp = tempfile::tempdir().unwrap();
    let first = tmp.path().join("first");
    let second = tmp.path().join("second");

    init_mem_root_once(&first).unwrap();
    init_mem_root_once(&second).unwrap();

    assert!(first.exists());
    assert!(
        !second.exists(),
        "second root must not be initialized after the first"
    );
}

#[test]
fn parse_pid_extracts_positive_pids_only() {
    assert_eq!(parse_pid("p12345"), Some(12345));
    assert_eq!(parse_pid("p0"), None);
    assert_eq!(parse_pid("pabc"), None);
    assert_eq!(parse_pid("other"), None);
}

#[test]
fn resolve_default_mem_root_returns_absolute_path() {
    let root = resolve_default_mem_root();
    assert!(root.is_absolute());
}

#[cfg(target_os = "linux")]
#[test]
fn available_bytes_reports_positive_free_space() {
    let tmp = tempfile::tempdir().unwrap();
    let free = available_bytes(tmp.path()).unwrap();
    assert!(free.is_some_and(|bytes| bytes > 0));
}

#[test]
fn secure_wipe_removes_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("fm_skill_security_abc");
    fs::create_dir_all(dir.join("scripts")).unwrap();
    fs::write(dir.join("SKILL.md"), "plaintext").unwrap();
    fs::write(dir.join("scripts/run.py"), "print('x')").unwrap();

    secure_wipe(&dir).unwrap();

    assert!(!dir.exists());
}

#[cfg(unix)]
#[test]
fn decrypted_dir_mode_is_0700() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("fm_skill_security_abc");
    write_package_entries(&[entry("SKILL.md", "# x")], &target).unwrap();
    let mode = fs::metadata(&target).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
}
