use super::{categories::Category, classify::Tag, Node};
use chrono::{Duration, Utc};
use std::path::PathBuf;

pub fn tree() -> Vec<Node> {
    let mut nodes = vec![Node {
        path: PathBuf::from("/home/demo"),
        name: "demo".into(),
        size: 0,
        files: 0,
        directory: true,
        atime: None,
        mtime: None,
        children: vec![],
        tag: Tag::Personal,
        reason: None,
        category: Category::Other,
    }];
    let projects = add(
        &mut nodes,
        0,
        "Projects",
        true,
        0,
        Tag::Personal,
        None,
        Category::ProjectsAndCode,
    );
    let backend = add(
        &mut nodes,
        projects,
        "backend",
        true,
        0,
        Tag::Personal,
        None,
        Category::ProjectsAndCode,
    );
    let target = add(
        &mut nodes,
        backend,
        "target",
        true,
        0,
        Tag::Junk,
        Some("Rebuilds automatically"),
        Category::CachesAndTemp,
    );
    add(
        &mut nodes,
        target,
        "debug-artifact.exe",
        false,
        16_000_000_000,
        Tag::Junk,
        Some("Rebuilds automatically"),
        Category::CachesAndTemp,
    );
    add(
        &mut nodes,
        backend,
        "source files",
        false,
        8_000_000_000,
        Tag::Personal,
        None,
        Category::ProjectsAndCode,
    );
    let frontend = add(
        &mut nodes,
        projects,
        "frontend",
        true,
        0,
        Tag::Personal,
        None,
        Category::ProjectsAndCode,
    );
    add(
        &mut nodes,
        frontend,
        "node_modules",
        true,
        9_000_000_000,
        Tag::Junk,
        Some("Rebuilds automatically"),
        Category::CachesAndTemp,
    );
    add(
        &mut nodes,
        frontend,
        "source files",
        false,
        3_000_000_000,
        Tag::Personal,
        None,
        Category::ProjectsAndCode,
    );
    let git = add(
        &mut nodes,
        projects,
        ".git",
        true,
        0,
        Tag::Personal,
        None,
        Category::ProjectsAndCode,
    );
    add(
        &mut nodes,
        git,
        "pack-objects",
        false,
        6_000_000_000,
        Tag::Personal,
        None,
        Category::ProjectsAndCode,
    );

    let modules = add(
        &mut nodes,
        0,
        "node_modules",
        true,
        0,
        Tag::Junk,
        Some("Rebuilds automatically"),
        Category::CachesAndTemp,
    );
    for (name, size) in [
        ("react", 8_000_000_000),
        ("chromium", 6_000_000_000),
        ("other packages", 4_000_000_000),
    ] {
        add(
            &mut nodes,
            modules,
            name,
            false,
            size,
            Tag::Junk,
            Some("Rebuilds automatically"),
            Category::CachesAndTemp,
        );
    }

    let downloads = add(
        &mut nodes,
        0,
        "Downloads",
        true,
        0,
        Tag::Personal,
        None,
        Category::Downloads,
    );
    let installers = add(
        &mut nodes,
        downloads,
        "old installers",
        true,
        0,
        Tag::Junk,
        Some("Installer no longer needed"),
        Category::Downloads,
    );
    add(
        &mut nodes,
        installers,
        "graphics-suite.exe",
        false,
        5_000_000_000,
        Tag::Junk,
        Some("Installer no longer needed"),
        Category::Downloads,
    );
    add(
        &mut nodes,
        installers,
        "sdk-bundle.zip",
        false,
        3_000_000_000,
        Tag::Junk,
        Some("Installer no longer needed"),
        Category::Downloads,
    );
    let archives = add(
        &mut nodes,
        downloads,
        "project archives",
        true,
        0,
        Tag::Unused,
        Some("Not touched in 6+ months"),
        Category::Downloads,
    );
    add(
        &mut nodes,
        archives,
        "old-work.zip",
        false,
        12_000_000_000,
        Tag::Unused,
        Some("Not touched in 6+ months"),
        Category::Downloads,
    );
    add(
        &mut nodes,
        archives,
        "backup.tar.gz",
        false,
        8_000_000_000,
        Tag::Unused,
        Some("Not touched in 6+ months"),
        Category::Downloads,
    );
    let datasets = add(
        &mut nodes,
        downloads,
        "datasets",
        true,
        0,
        Tag::Personal,
        None,
        Category::Downloads,
    );
    add(
        &mut nodes,
        datasets,
        "training-data.bin",
        false,
        36_000_000_000,
        Tag::Personal,
        None,
        Category::Downloads,
    );

    let movies = add(
        &mut nodes,
        0,
        "Movies",
        true,
        0,
        Tag::Personal,
        None,
        Category::MediaAndArchives,
    );
    add(
        &mut nodes,
        movies,
        "interstellar-4k.mkv",
        false,
        45_000_000_000,
        Tag::Personal,
        None,
        Category::MediaAndArchives,
    );
    let documentaries = add(
        &mut nodes,
        movies,
        "documentaries",
        true,
        0,
        Tag::Personal,
        None,
        Category::MediaAndArchives,
    );
    add(
        &mut nodes,
        documentaries,
        "ocean.mkv",
        false,
        24_000_000_000,
        Tag::Personal,
        None,
        Category::MediaAndArchives,
    );
    add(
        &mut nodes,
        documentaries,
        "wildlife.mkv",
        false,
        16_000_000_000,
        Tag::Personal,
        None,
        Category::MediaAndArchives,
    );
    let originals = add(
        &mut nodes,
        movies,
        "camera-originals",
        true,
        0,
        Tag::Personal,
        None,
        Category::MediaAndArchives,
    );
    add(
        &mut nodes,
        originals,
        "shoot-01.mov",
        false,
        20_000_000_000,
        Tag::Personal,
        None,
        Category::MediaAndArchives,
    );
    add(
        &mut nodes,
        originals,
        "shoot-02.mov",
        false,
        15_000_000_000,
        Tag::Personal,
        None,
        Category::MediaAndArchives,
    );

    let cache = add(
        &mut nodes,
        0,
        ".cache",
        true,
        0,
        Tag::Junk,
        Some("Rebuilds automatically"),
        Category::CachesAndTemp,
    );
    for (name, size) in [
        ("browser", 5_000_000_000),
        ("compiler", 4_000_000_000),
        ("thumbnails", 3_000_000_000),
    ] {
        add(
            &mut nodes,
            cache,
            name,
            false,
            size,
            Tag::Junk,
            Some("Rebuilds automatically"),
            Category::CachesAndTemp,
        );
    }

    for i in (1..nodes.len()).rev() {
        let parent = nodes[i]
            .path
            .parent()
            .and_then(|p| nodes.iter().position(|n| n.path == p))
            .unwrap_or(0);
        nodes[parent].size = nodes[parent].size.saturating_add(nodes[i].size);
        nodes[parent].files = nodes[parent].files.saturating_add(nodes[i].files);
    }
    nodes
}

fn add(
    nodes: &mut Vec<Node>,
    parent: usize,
    name: &str,
    directory: bool,
    size: u64,
    tag: Tag,
    reason: Option<&str>,
    category: Category,
) -> usize {
    let index = nodes.len();
    nodes[parent].children.push(index);
    let days_ago = match tag {
        Tag::Unused => 220,
        Tag::Junk if category == Category::Downloads => 45,
        Tag::Junk => 5,
        _ => 14,
    };
    let touched = Some(Utc::now() - Duration::days(days_ago));
    nodes.push(Node {
        path: nodes[parent].path.join(name),
        name: name.into(),
        size,
        files: u64::from(!directory),
        directory,
        atime: touched,
        mtime: touched,
        children: vec![],
        tag,
        reason: reason.map(str::to_owned),
        category,
    });
    index
}
