use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Category {
    ProjectsAndCode,
    Downloads,
    CachesAndTemp,
    MediaAndArchives,
    Other,
}

pub fn category(path: &Path) -> Category {
    let s = path.to_string_lossy().to_lowercase();
    let n = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if s.contains("download") {
        Category::Downloads
    } else if [
        "node_modules",
        "target",
        ".cache",
        "temp",
        ".venv",
        "__pycache__",
    ]
    .contains(&n.as_str())
    {
        Category::CachesAndTemp
    } else if [".rs", ".py", ".js", ".toml", ".git"]
        .iter()
        .any(|e| s.ends_with(e))
    {
        Category::ProjectsAndCode
    } else if [".mp4", ".mkv", ".mp3", ".zip", ".tar", ".gz"]
        .iter()
        .any(|e| s.ends_with(e))
    {
        Category::MediaAndArchives
    } else {
        Category::Other
    }
}
