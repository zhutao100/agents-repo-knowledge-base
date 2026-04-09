use crate::error::KbError;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct RepoPath(String);

impl RepoPath {
    pub fn parse(input: &str) -> Result<Self, KbError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(KbError::invalid_argument("path must not be empty"));
        }
        if trimmed == "." {
            return Ok(Self(String::new()));
        }
        if trimmed.starts_with('/') {
            return Err(KbError::invalid_argument("absolute paths are not allowed")
                .with_detail("path", trimmed));
        }
        if looks_like_windows_abs_path(trimmed) {
            return Err(KbError::invalid_argument("absolute paths are not allowed")
                .with_detail("path", trimmed));
        }
        if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
            return Err(
                KbError::invalid_argument("path contains invalid characters")
                    .with_detail("path", trimmed),
            );
        }

        let normalized = trimmed.replace('\\', "/");
        let mut out: Vec<&str> = Vec::new();
        for part in normalized.split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            if part == ".." {
                return Err(KbError::invalid_argument("path must not contain '..'")
                    .with_detail("path", trimmed));
            }
            out.push(part);
        }

        Ok(Self(out.join("/")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }
}

fn looks_like_windows_abs_path(path: &str) -> bool {
    let mut chars = path.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let Some(second) = chars.next() else {
        return false;
    };
    let Some(third) = chars.next() else {
        return false;
    };

    (first.is_ascii_alphabetic() && second == ':' && (third == '\\' || third == '/'))
        || (first == '\\' && second == '\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_rejects_parent_dir_escape() {
        assert!(RepoPath::parse("../x").is_err());
        assert!(RepoPath::parse("a/../../x").is_err());
    }

    #[test]
    fn normalize_rejects_absolute_paths() {
        assert!(RepoPath::parse("/abs/path").is_err());
        assert!(RepoPath::parse("C:\\abs\\path").is_err());
    }

    #[test]
    fn normalize_accepts_repo_relative_paths() {
        assert_eq!(
            RepoPath::parse("src/lib.rs").unwrap().as_str(),
            "src/lib.rs"
        );
        assert_eq!(
            RepoPath::parse("./src/./lib.rs").unwrap().as_str(),
            "src/lib.rs"
        );
        assert_eq!(
            RepoPath::parse("src\\\\lib.rs").unwrap().as_str(),
            "src/lib.rs"
        );
    }
}
