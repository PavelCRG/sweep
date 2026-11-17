use super::{categories, classify, Node};
use chrono::{DateTime, Utc};
use std::{collections::HashMap, fs, path::Path, time::SystemTime};

fn date(t: std::io::Result<SystemTime>) -> Option<DateTime<Utc>> {
    t.ok().map(DateTime::<Utc>::from)
}

pub fn scan_many(roots: &[std::path::PathBuf]) -> std::io::Result<Vec<Node>> {
    let mut roots: Vec<_> = roots
        .iter()
        .map(fs::canonicalize)
        .collect::<std::io::Result<_>>()?;
    if roots.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "at least one scan directory is required",
        ));
    }
    for root in &mut roots {
        *root = displayable_path(std::mem::take(root));
    }
    roots.sort();
    roots.dedup();
    if roots.len() == 1 {
        return scan_excluding(&roots[0], &[]);
    }
    let mut all = vec![Node {
        path: std::path::PathBuf::from("<all disks>"),
        name: "All disks".into(),
        size: 0,
        files: 0,
        directory: true,
        atime: None,
        mtime: None,
        children: vec![],
        tag: classify::Tag::Personal,
        reason: None,
        category: categories::Category::Other,
    }];
    for root in &roots {
        let nested: Vec<_> = roots
            .iter()
            .filter(|other| *other != root && other.starts_with(root))
            .cloned()
            .collect();
        let mut tree = scan_excluding(root, &nested)?;
        let offset = all.len();
        for node in &mut tree {
            for child in &mut node.children {
                *child += offset;
            }
        }
        all[0].children.push(offset);
        all[0].size = all[0].size.saturating_add(tree[0].size);
        all[0].files = all[0].files.saturating_add(tree[0].files);
        all.extend(tree);
    }
    Ok(all)
}

fn scan_excluding(root: &Path, excluded: &[std::path::PathBuf]) -> std::io::Result<Vec<Node>> {
    let root_meta = fs::metadata(root)?;
    if !root_meta.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("scan path is not a directory: {}", root.display()),
        ));
    }
    let root_name = root
        .file_name()
        .unwrap_or(root.as_os_str())
        .to_string_lossy()
        .into_owned();
    let (mut root_tag, mut root_reason) = classify::classify(
        root,
        &root_name,
        root_meta.is_dir(),
        date(root_meta.accessed()),
        date(root_meta.modified()),
    );
    if root_tag == classify::Tag::Unused {
        root_tag = classify::Tag::Personal;
        root_reason = None;
    }
    let mut nodes = vec![Node {
        path: root.to_path_buf(),
        name: root_name,
        size: 0,
        files: 0,
        directory: root_meta.is_dir(),
        atime: date(root_meta.accessed()),
        mtime: date(root_meta.modified()),
        children: vec![],
        tag: root_tag,
        reason: root_reason,
        category: categories::Category::Other,
    }];
    let mut index = HashMap::from([(root.to_path_buf(), 0usize)]);
    let mut excluded = excluded.to_vec();
    if cfg!(unix) {
        excluded.extend(
            ["/proc", "/sys", "/dev", "/run"]
                .iter()
                .map(std::path::PathBuf::from),
        );
    }
    for entry in jwalk::WalkDir::new(root)
        .skip_hidden(false)
        .process_read_dir(move |_, _, _, entries| {
            for entry in entries {
                if let Ok(entry) = entry {
                    if excluded.iter().any(|path| entry.path() == *path) {
                        entry.read_children_path = None;
                    }
                }
            }
        })
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let is_dir = meta.is_dir();
        let parent = path
            .parent()
            .and_then(|p| index.get(p))
            .copied()
            .unwrap_or(0);
        let i = nodes.len();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let (tag, reason) = match nodes[parent].tag {
            classify::Tag::System | classify::Tag::Junk => {
                (nodes[parent].tag, nodes[parent].reason.clone())
            }
            _ => classify::classify(
                &path,
                &name,
                is_dir,
                date(meta.accessed()),
                date(meta.modified()),
            ),
        };
        nodes.push(Node {
            path: path.clone(),
            name,
            size: if is_dir { 0 } else { meta.len() },
            files: u64::from(!is_dir),
            directory: is_dir,
            atime: date(meta.accessed()),
            mtime: date(meta.modified()),
            children: vec![],
            tag,
            reason,
            category: categories::category(&path),
        });
        nodes[parent].children.push(i);
        index.insert(path, i);
    }
    for i in (1..nodes.len()).rev() {
        let p = nodes[i]
            .path
            .parent()
            .and_then(|p| index.get(p))
            .copied()
            .unwrap_or(0);
        nodes[p].size = nodes[p].size.saturating_add(nodes[i].size);
        nodes[p].files = nodes[p].files.saturating_add(nodes[i].files);
    }
    Ok(nodes)
}

fn displayable_path(path: std::path::PathBuf) -> std::path::PathBuf {
    let Some(path) = path.to_str() else {
        return path;
    };
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        std::path::PathBuf::from(format!(r"\\{unc}"))
    } else if let Some(path) = path.strip_prefix(r"\\?\") {
        std::path::PathBuf::from(path)
    } else {
        std::path::PathBuf::from(path)
    }
}
