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
    let mut download = false;
    let mut cache = false;
    let mut project = false;
    let mut media = false;
    for component in path.components() {
        let component = component.as_os_str().to_string_lossy();
        download |= contains_ascii_case_insensitive(&component, "download");
        cache |= [
            "node_modules",
            "target",
            ".cache",
            "temp",
            ".venv",
            "__pycache__",
        ]
        .iter()
        .any(|name| component.eq_ignore_ascii_case(name));
        project |= [
            "project",
            "projects",
            "src",
            "source",
            "developer",
            "development",
            "code",
            "repositories",
            "repo",
        ]
        .iter()
        .any(|name| component.eq_ignore_ascii_case(name));
        media |= [
            "media", "photos", "pictures", "videos", "movies", "music", "archives",
        ]
        .iter()
        .any(|name| component.eq_ignore_ascii_case(name));
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    if download {
        Category::Downloads
    } else if cache {
        Category::CachesAndTemp
    } else if project
        || [".rs", ".py", ".js", ".toml", ".git"]
            .iter()
            .any(|extension| ends_with_ascii_case_insensitive(&name, extension))
    {
        Category::ProjectsAndCode
    } else if media
        || [".mp4", ".mkv", ".mp3", ".zip", ".tar", ".gz"]
            .iter()
            .any(|extension| ends_with_ascii_case_insensitive(&name, extension))
    {
        Category::MediaAndArchives
    } else {
        Category::Other
    }
}

fn contains_ascii_case_insensitive(value: &str, needle: &str) -> bool {
    value
        .as_bytes()
        .windows(needle.len())
        .any(|part| part.eq_ignore_ascii_case(needle.as_bytes()))
}

fn ends_with_ascii_case_insensitive(value: &str, suffix: &str) -> bool {
    value
        .as_bytes()
        .strip_suffix(suffix.as_bytes())
        .is_some_and(|part| part.eq_ignore_ascii_case(suffix.as_bytes()))
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
        assert_eq!(
            category(Path::new("D:/DOWNLOADS/archive.ZIP")),
            Category::Downloads
        );
        assert_eq!(
            category(Path::new("D:/Projects/Target/BUILD.EXE")),
            Category::CachesAndTemp
        );
        assert_eq!(
            category(Path::new("/home/user/VIDEOS/CLIP.MKV")),
            Category::MediaAndArchives
        );
    }
}
