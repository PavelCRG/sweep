use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::OnceLock};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Tag {
    Junk,
    Unused,
    System,
    Personal,
}

pub fn system_roots() -> &'static [String] {
    static ROOTS: OnceLock<Vec<String>> = OnceLock::new();
    ROOTS.get_or_init(|| {
        if cfg!(windows) {
            ["WINDIR", "ProgramFiles", "ProgramFiles(x86)", "ProgramData"]
                .iter()
                .filter_map(std::env::var_os)
                .map(|p| p.to_string_lossy().to_lowercase())
                .collect()
        } else {
            ["/usr", "/etc", "/sys", "/proc", "/var/lib/docker"]
                .iter()
                .map(|p| p.to_string())
                .collect()
        }
    })
}

fn same_or_below(path: &str, root: &str) -> bool {
    let path = path.trim_end_matches(['\\', '/']);
    let root = root.trim_end_matches(['\\', '/']);
    path == root
        || root
            .strip_prefix(path)
            .is_some_and(|tail| tail.starts_with(['\\', '/']))
}

pub fn is_system_path(path: &Path) -> bool {
    let path = path.to_string_lossy().to_lowercase();
    system_roots().iter().any(|root| same_or_below(&path, root))
}

pub fn is_system_area(path: &Path) -> bool {
    let path = path.to_string_lossy().to_lowercase();
    system_roots()
        .iter()
        .any(|root| same_or_below(&path, root) || same_or_below(root, &path))
}

pub fn classify(
    path: &Path,
    name: &str,
    is_dir: bool,
    atime: Option<DateTime<Utc>>,
    mtime: Option<DateTime<Utc>>,
) -> (Tag, Option<String>) {
    if is_system_path(path) {
        return (
            Tag::System,
            Some("Managed by system, deletion blocked".into()),
        );
    }
    if is_dir
        && [
            "node_modules",
            "target",
            "__pycache__",
            ".venv",
            ".next",
            ".cache",
            "temp",
        ]
        .contains(&name.to_lowercase().as_str())
    {
        return (Tag::Junk, Some("Rebuilds automatically".into()));
    }
    let downloads = path.components().any(|c| {
        c.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("downloads")
    });
    let ext = [".exe", ".msi", ".deb", ".appimage", ".zip", ".tar.gz"]
        .iter()
        .any(|x| name.to_lowercase().ends_with(x));
    if downloads && ext && mtime.is_some_and(|t| t < Utc::now() - Duration::days(30)) {
        return (Tag::Junk, Some("Installer no longer needed".into()));
    }
    if matches!((atime, mtime), (Some(a), Some(m)) if a < Utc::now() - Duration::days(180) && m < Utc::now() - Duration::days(180))
    {
        return (Tag::Unused, Some("Not touched in 6+ months".into()));
    }
    (Tag::Personal, None)
}
