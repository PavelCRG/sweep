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
    let has_component = |names: &[&str]| s.split(['\\', '/']).any(|part| names.contains(&part));
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
        || has_component(&[
            "node_modules",
            "target",
            ".cache",
            "temp",
            ".venv",
            "__pycache__",
        ])
    {
        Category::CachesAndTemp
    } else if has_component(&[
        "project",
        "projects",
        "src",
        "source",
        "developer",
        "development",
        "code",
        "repositories",
        "repo",
    ]) || [".rs", ".py", ".js", ".toml", ".git"]
        .iter()
        .any(|e| s.ends_with(e))
    {
        Category::ProjectsAndCode
    } else if has_component(&[
        "media", "photos", "pictures", "videos", "movies", "music", "archives",
    ]) || [".mp4", ".mkv", ".mp3", ".zip", ".tar", ".gz"]
        .iter()
        .any(|e| s.ends_with(e))
    {
        Category::MediaAndArchives
    } else {
        Category::Other
    }
}

#[cfg(test)]
mod tests {
    use super::{category, Category};
    use std::path::Path;

    #[test]
    fn category_names_apply_to_descendants() {
        assert_eq!(
            category(Path::new("/home/user/Projects/api/src/main.rs")),
            Category::ProjectsAndCode
        );
        assert_eq!(
            category(Path::new("/home/user/Downloads/old/installer.exe")),
            Category::Downloads
        );
        assert_eq!(
            category(Path::new("/home/user/Videos/clip.mkv")),
            Category::MediaAndArchives
        );
        assert_eq!(
            category(Path::new("/home/user/Projects/api/target/debug/app")),
            Category::CachesAndTemp
        );
    }
}
