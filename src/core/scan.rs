use super::{categories, classify, Node};
use chrono::{DateTime, Utc};
use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScanProgress {
    pub visited: u64,
    pub skipped: u64,
}

#[derive(Clone, Debug)]
pub struct ScanEntry {
    pub parent: Option<std::path::PathBuf>,
    pub node: Node,
}

fn date(t: std::io::Result<SystemTime>) -> Option<DateTime<Utc>> {
    t.ok().map(DateTime::<Utc>::from)
}

pub fn scan_many_with_progress<F>(
    roots: &[std::path::PathBuf],
    cancelled: &AtomicBool,
    mut progress: F,
) -> std::io::Result<Vec<Node>>
where
    F: FnMut(ScanProgress),
{
    let mut state = ScanProgress::default();
    let mut no_entries = |_| {};
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
        let result = scan_excluding(
            &roots[0],
            &[],
            cancelled,
            &mut state,
            &mut progress,
            &mut no_entries,
            true,
        );
        progress(state);
        return result;
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
        let mut tree = scan_excluding(
            root,
            &nested,
            cancelled,
            &mut state,
            &mut progress,
            &mut no_entries,
            true,
        )?;
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
    progress(state);
    Ok(all)
}

pub fn scan_one_with_updates<P, B>(
    root: &Path,
    cancelled: &AtomicBool,
    mut progress: P,
    mut on_batch: B,
) -> std::io::Result<ScanProgress>
where
    P: FnMut(ScanProgress),
    B: FnMut(Vec<ScanEntry>),
{
    let root = displayable_path(fs::canonicalize(root)?);
    let mut state = ScanProgress::default();
    let mut batch = Vec::with_capacity(64);
    let result = {
        let mut emit = |entry: ScanEntry| {
            if entry.parent.is_none() {
                on_batch(vec![entry]);
            } else {
                batch.push(entry);
                if batch.len() == 64 {
                    on_batch(std::mem::take(&mut batch));
                }
            }
        };
        scan_excluding(
            &root,
            &[],
            cancelled,
            &mut state,
            &mut progress,
            &mut emit,
            false,
        )
    };
    if !batch.is_empty() {
        on_batch(batch);
    }
    progress(state);
    result.map(|_| state)
}

fn scan_excluding<F, E>(
    root: &Path,
    excluded: &[std::path::PathBuf],
    cancelled: &AtomicBool,
    state: &mut ScanProgress,
    progress: &mut F,
    on_entry: &mut E,
    build_tree: bool,
) -> std::io::Result<Vec<Node>>
where
    F: FnMut(ScanProgress),
    E: FnMut(ScanEntry),
{
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
    let root_node = Node {
        path: root.to_path_buf(),
        name: root_name,
        size: 0,
        files: 0,
        directory: root_meta.is_dir(),
        atime: date(root_meta.accessed()),
        mtime: date(root_meta.modified()),
        children: vec![],
        tag: root_tag,
        reason: root_reason.clone(),
        category: categories::Category::Other,
    };
    let mut nodes = vec![root_node.clone()];
    let mut tags = HashMap::new();
    if !build_tree {
        tags.insert(root.to_path_buf(), (root_tag, root_reason.clone()));
        on_entry(ScanEntry {
            parent: None,
            node: root_node,
        });
    }
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
            for entry in entries.iter_mut().flatten() {
                if excluded.iter().any(|path| entry.path() == *path) {
                    entry.read_children_path = None;
                }
            }
        })
        .into_iter()
    {
        if cancelled.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "scan cancelled",
            ));
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                state.skipped = state.skipped.saturating_add(1);
                progress(*state);
                continue;
            }
        };
        let path = entry.path();
        if path == root {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(meta) => meta,
            Err(_) => {
                state.skipped = state.skipped.saturating_add(1);
                progress(*state);
                continue;
            }
        };
        state.visited = state.visited.saturating_add(1);
        progress(*state);
        if cancelled.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "scan cancelled",
            ));
        }
        let is_dir = meta.is_dir();
        let raw_parent = path.parent().unwrap_or(root);
        let parent = index.get(raw_parent).copied().unwrap_or(0);
        let (parent_tag, parent_reason) = if build_tree {
            (nodes[parent].tag, nodes[parent].reason.clone())
        } else {
            tags.get(raw_parent)
                .cloned()
                .unwrap_or((root_tag, root_reason.clone()))
        };
        let parent_path = if build_tree {
            nodes[parent].path.clone()
        } else {
            raw_parent.to_path_buf()
        };
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let (tag, reason) = match parent_tag {
            classify::Tag::System | classify::Tag::Junk => (parent_tag, parent_reason),
            _ => classify::classify(
                &path,
                &name,
                is_dir,
                date(meta.accessed()),
                date(meta.modified()),
            ),
        };
        let node = Node {
            path: path.clone(),
            name,
            size: if is_dir { 0 } else { meta.len() },
            files: u64::from(!is_dir),
            directory: is_dir,
            atime: date(meta.accessed()),
            mtime: date(meta.modified()),
            children: vec![],
            tag,
            reason: reason.clone(),
            category: categories::category(&path),
        };
        if build_tree {
            let i = nodes.len();
            nodes.push(node);
            nodes[parent].children.push(i);
            index.insert(path, i);
        } else {
            tags.insert(path, (tag, reason.clone()));
            on_entry(ScanEntry {
                parent: Some(parent_path),
                node,
            });
        }
    }
    if build_tree {
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

#[cfg(test)]
mod tests {
    use super::{scan_many_with_progress, scan_one_with_updates};
    use std::{
        fs,
        sync::atomic::{AtomicBool, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn scan_stops_when_cancelled_from_progress_callback() {
        let root = std::env::temp_dir().join(format!(
            "sweep-cancel-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        for i in 0..8 {
            fs::write(root.join(format!("{i}.txt")), b"data").unwrap();
        }
        let cancelled = AtomicBool::new(false);
        let result = scan_many_with_progress(std::slice::from_ref(&root), &cancelled, |_| {
            cancelled.store(true, Ordering::Relaxed);
        });
        fs::remove_dir_all(root).unwrap();
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Interrupted);
    }

    #[test]
    fn streaming_scan_emits_live_entries_with_parent_paths_and_counts() {
        let root = std::env::temp_dir().join(format!(
            "sweep-stream-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let folder = root.join("folder");
        let file = folder.join("sample.bin");
        fs::create_dir_all(&folder).unwrap();
        fs::write(&file, [7; 123]).unwrap();
        let canonical_root = super::displayable_path(fs::canonicalize(&root).unwrap());
        let cancelled = AtomicBool::new(false);
        let mut entries = Vec::new();
        let progress = scan_one_with_updates(
            &root,
            &cancelled,
            |_| {},
            |batch| {
                entries.extend(batch);
            },
        )
        .unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(progress.visited, 2);
        assert_eq!(entries[0].parent, None);
        assert_eq!(entries[0].node.path, canonical_root);
        let folder_entry = entries
            .iter()
            .find(|entry| entry.node.path.ends_with("folder"))
            .unwrap();
        let file_entry = entries
            .iter()
            .find(|entry| entry.node.path.ends_with("sample.bin"))
            .unwrap();
        assert_eq!(
            file_entry.parent.as_deref(),
            Some(folder_entry.node.path.as_path())
        );
        assert_eq!(file_entry.node.size, 123);
    }
}
