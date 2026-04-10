use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TempRepo {
    path: PathBuf,
}

impl TempRepo {
    pub fn new(prefix: &str) -> Self {
        Self {
            path: temp_repo_dir(prefix),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

impl std::ops::Deref for TempRepo {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.path()
    }
}

impl AsRef<Path> for TempRepo {
    fn as_ref(&self) -> &Path {
        self.path()
    }
}

pub fn temp_repo_dir(prefix: &str) -> PathBuf {
    for _ in 0..1000 {
        let mut dir = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        dir.push(format!("{prefix}{}-{nanos}-{n}", std::process::id()));
        match std::fs::create_dir(&dir) {
            Ok(()) => return dir,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("create temp repo dir: {e}"),
        }
    }

    panic!("failed to allocate a unique temp repo directory after many attempts");
}
