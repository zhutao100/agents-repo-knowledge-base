#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiffSource {
    Staged,
    Worktree,
    Commit(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffSourceParseError {
    Empty,
    Unknown,
    MissingCommit,
}

impl DiffSource {
    pub fn parse(input: &str) -> Result<Self, DiffSourceParseError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(DiffSourceParseError::Empty);
        }

        match trimmed {
            "staged" => Ok(DiffSource::Staged),
            "worktree" => Ok(DiffSource::Worktree),
            _ => {
                let Some(commit) = trimmed.strip_prefix("commit:") else {
                    return Err(DiffSourceParseError::Unknown);
                };
                if commit.is_empty() {
                    return Err(DiffSourceParseError::MissingCommit);
                }
                Ok(DiffSource::Commit(commit.to_string()))
            }
        }
    }

    pub fn as_selector(&self) -> &str {
        match self {
            DiffSource::Staged => "staged",
            DiffSource::Worktree => "worktree",
            DiffSource::Commit(_) => "commit:<sha>",
        }
    }

    pub fn as_git_spec(&self) -> Option<&str> {
        match self {
            DiffSource::Commit(sha) => Some(sha.as_str()),
            _ => None,
        }
    }

    pub fn as_display(&self) -> String {
        match self {
            DiffSource::Staged => "staged".to_string(),
            DiffSource::Worktree => "worktree".to_string(),
            DiffSource::Commit(sha) => format!("commit:{sha}"),
        }
    }
}
