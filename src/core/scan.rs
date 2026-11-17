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
        let result = scan_excluding(&roots[0], &[], cancelled, &mut state, &mut progress);
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
        let mut tree = scan_excluding(root, &nested, cancelled, &mut state, &mut progress)?;
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

fn scan_excluding<F>(
    root: &Path,
    excluded: &[std::path::PathBuf],
    cancelled: &AtomicBool,
    state: &mut ScanProgress,
    progress: &mut F,
) -> std::io::Result<Vec<Node>>
where
    F: FnMut(ScanProgress),
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

#[cfg(test)]
mod tests {
    use super::scan_many_with_progress;
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
}
