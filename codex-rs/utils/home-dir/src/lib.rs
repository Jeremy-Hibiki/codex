use codex_utils_absolute_path::AbsolutePathBuf;
use dirs::home_dir;
use std::path::PathBuf;

/// Environment variable that overrides the Grevo home directory.
pub const GREVO_HOME_ENV_VAR: &str = "GREVO_HOME";

/// Legacy environment variable still honored as an override for the home
/// directory so pre-rename setups and automation keep working.
pub const LEGACY_CODEX_HOME_ENV_VAR: &str = "CODEX_HOME";

/// Returns the path to the Grevo configuration directory, which can be
/// specified by the `GREVO_HOME` environment variable (the legacy
/// `CODEX_HOME` is still honored). If neither is set, defaults to `~/.grevo`.
///
/// - If an override is set, the value must exist and be a directory. The
///   value will be canonicalized and this function will Err otherwise.
/// - If no override is set, this function does not verify that the directory
///   exists.
pub fn find_codex_home() -> std::io::Result<AbsolutePathBuf> {
    let codex_home_env = override_home_env();
    find_codex_home_from_env(codex_home_env.as_deref())
}

fn override_home_env() -> Option<String> {
    [GREVO_HOME_ENV_VAR, LEGACY_CODEX_HOME_ENV_VAR]
        .iter()
        .find_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|val| !val.is_empty())
                .map(|val| format!("{key}={val}"))
        })
}

fn find_codex_home_from_env(codex_home_env: Option<&str>) -> std::io::Result<AbsolutePathBuf> {
    // Honor the home-directory override environment variables to allow users
    // (and tests) to override the default location.
    match codex_home_env {
        Some(env) => {
            let (key, val) = env
                .split_once('=')
                .unwrap_or((LEGACY_CODEX_HOME_ENV_VAR, env));
            let path = PathBuf::from(val);
            let metadata = std::fs::metadata(&path).map_err(|err| match err.kind() {
                std::io::ErrorKind::NotFound => std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("{key} points to {val:?}, but that path does not exist"),
                ),
                _ => {
                    std::io::Error::new(err.kind(), format!("failed to read {key} {val:?}: {err}"))
                }
            })?;

            if !metadata.is_dir() {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("{key} points to {val:?}, but that path is not a directory"),
                ))
            } else {
                let canonical = path.canonicalize().map_err(|err| {
                    std::io::Error::new(
                        err.kind(),
                        format!("failed to canonicalize {key} {val:?}: {err}"),
                    )
                })?;
                AbsolutePathBuf::from_absolute_path(canonical)
            }
        }
        None => {
            let mut p = home_dir().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Could not find home directory",
                )
            })?;
            p.push(".grevo");
            AbsolutePathBuf::from_absolute_path(p)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::find_codex_home_from_env;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use dirs::home_dir;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::io::ErrorKind;
    use tempfile::TempDir;

    #[test]
    fn find_codex_home_env_missing_path_is_fatal() {
        let temp_home = TempDir::new().expect("temp home");
        let missing = temp_home.path().join("missing-codex-home");
        let missing_str = missing
            .to_str()
            .expect("missing codex home path should be valid utf-8");

        let err = find_codex_home_from_env(Some(missing_str)).expect_err("missing GREVO_HOME");
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(
            err.to_string().contains("points to"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn find_codex_home_env_file_path_is_fatal() {
        let temp_home = TempDir::new().expect("temp home");
        let file_path = temp_home.path().join("codex-home.txt");
        fs::write(&file_path, "not a directory").expect("write temp file");
        let file_str = file_path
            .to_str()
            .expect("file codex home path should be valid utf-8");

        let err = find_codex_home_from_env(Some(file_str)).expect_err("file GREVO_HOME");
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("not a directory"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn find_codex_home_env_valid_directory_canonicalizes() {
        let temp_home = TempDir::new().expect("temp home");
        let temp_str = temp_home
            .path()
            .to_str()
            .expect("temp codex home path should be valid utf-8");

        let resolved = find_codex_home_from_env(Some(temp_str)).expect("valid GREVO_HOME");
        let expected = temp_home
            .path()
            .canonicalize()
            .expect("canonicalize temp home");
        let expected = AbsolutePathBuf::from_absolute_path(expected).expect("absolute home");
        assert_eq!(resolved, expected);
    }

    #[test]
    fn find_codex_home_without_env_uses_default_home_dir() {
        let resolved =
            find_codex_home_from_env(/*codex_home_env*/ None).expect("default GREVO_HOME");
        let mut expected = home_dir().expect("home dir");
        expected.push(".grevo");
        let expected = AbsolutePathBuf::from_absolute_path(expected).expect("absolute home");
        assert_eq!(resolved, expected);
    }
}
