use crate::error::KbError;

pub fn normalize_path_prefix(input: &str) -> Result<String, KbError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(KbError::invalid_argument("path prefix must not be empty"));
    }
    if trimmed.starts_with('/') {
        return Err(
            KbError::invalid_argument("path prefix must be repo-relative")
                .with_detail("prefix", trimmed),
        );
    }
    if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(
            KbError::invalid_argument("path prefix contains invalid characters")
                .with_detail("prefix", trimmed),
        );
    }

    let ends_with_slash = trimmed.ends_with('/');
    let normalized = trimmed.replace('\\', "/");

    let mut parts = Vec::new();
    for part in normalized.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(
                KbError::invalid_argument("path prefix must not contain '..'")
                    .with_detail("prefix", trimmed),
            );
        }
        parts.push(part);
    }

    let mut out = parts.join("/");
    if ends_with_slash && !out.is_empty() {
        out.push('/');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_path_prefix_preserves_trailing_slash() {
        assert_eq!(normalize_path_prefix("src/").unwrap(), "src/");
        assert_eq!(normalize_path_prefix("src/lib.rs").unwrap(), "src/lib.rs");
    }
}
