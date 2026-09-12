//! Tests for [`crate::sandbox_policy`].

use crate::sandbox_policy::ensure_encrypted_skill_sandbox;

#[test]
fn ensure_encrypted_skill_sandbox_gates_engaged_execution() {
    assert!(ensure_encrypted_skill_sandbox(true, false, /*allow_sandbox_bypass*/ false).is_err());
    assert!(ensure_encrypted_skill_sandbox(true, false, /*allow_sandbox_bypass*/ true).is_ok());
    assert!(ensure_encrypted_skill_sandbox(true, true, /*allow_sandbox_bypass*/ false).is_ok());
    assert!(ensure_encrypted_skill_sandbox(false, false, /*allow_sandbox_bypass*/ false).is_ok());
}
