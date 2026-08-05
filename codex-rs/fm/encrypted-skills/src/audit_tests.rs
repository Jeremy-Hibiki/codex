use super::*;

#[test]
fn serialize_contains_event_type_and_fields() {
    let event = AuditEvent::Decryption {
        session_id: "t1".into(),
        skill_name: "secret-skill".into(),
        cache_hit: false,
    };
    let line = serialize(&event);
    assert!(line.contains("\"event\":\"decryption\""));
    assert!(line.contains("\"session_id\":\"t1\""));
    assert!(line.contains("\"skill_name\":\"secret-skill\""));
    assert!(line.contains("\"cache_hit\":false"));
}

#[test]
fn write_event_appends_one_line() {
    let event = AuditEvent::Rehydration {
        session_id: "t1".into(),
        token_count: 3,
    };
    let mut buf = Vec::new();
    write_event(&mut buf, &event).unwrap();
    write_event(&mut buf, &event).unwrap();
    let text = String::from_utf8(buf).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert!(
        text.lines()
            .all(|line| line.contains("\"event\":\"rehydration\""))
    );
}

#[test]
fn audit_lines_never_contain_plaintext() {
    let event = AuditEvent::Tokenization {
        session_id: "t1".into(),
        skill_name: "secret-skill".into(),
    };
    let line = serialize(&event);
    assert!(!line.contains("SKILL.md content"));
    assert!(!line.contains("/dev/shm/fm-agent-security"));
}

#[test]
fn event_type_and_session_id_helpers() {
    let event = AuditEvent::Blocked {
        session_id: "t1".into(),
        tool: "shell".into(),
        reason: "direct read blocked".into(),
    };
    assert_eq!(event.event_type(), "blocked");
    assert_eq!(event.session_id(), "t1");
}

#[test]
fn file_audit_sink_writes_jsonl_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("audit.log");
    let sink = FileAuditSink::new(path.clone()).unwrap();
    sink.emit(AuditEvent::Decryption {
        session_id: "t1".into(),
        skill_name: "secret".into(),
        cache_hit: false,
    });
    sink.emit(AuditEvent::Rehydration {
        session_id: "t1".into(),
        token_count: 2,
    });

    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert!(text.lines().all(|line| line.contains("\"event\":")));
}

#[test]
fn file_audit_sink_rotates_at_size_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("audit.log");
    let sink = FileAuditSink::new_with_limit(path.clone(), 64).unwrap();
    for _ in 0..10 {
        sink.emit(AuditEvent::Decryption {
            session_id: "t1".into(),
            skill_name: "secret".into(),
            cache_hit: false,
        });
    }

    assert!(path.exists());
    let rotated = tmp.path().join("audit.log.1");
    assert!(rotated.exists(), "expected rotated audit file");
}
