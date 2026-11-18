use super::Node;

pub fn largest_children(nodes: &[Node], parent: usize) -> Vec<usize> {
    let mut out = nodes[parent].children.clone();
    out.sort_by_key(|&i| std::cmp::Reverse(nodes[i].size));
    out
}

pub fn largest_files(nodes: &[Node], parent: usize) -> Vec<usize> {
    let mut pending = nodes[parent].children.clone();
    let mut files = Vec::new();
    while let Some(i) = pending.pop() {
        if nodes[i].directory {
            pending.extend(nodes[i].children.iter().copied());
        } else {
            files.push(i);
        }
    }
    files.sort_unstable_by_key(|&i| std::cmp::Reverse(nodes[i].size));
    // shortcut: show only the largest 100 files, increase if users need deeper results.
    files.truncate(100);
    files
}

pub fn cleanup_candidates(nodes: &[Node], parent: usize) -> Vec<usize> {
    let mut pending = nodes[parent].children.clone();
    let mut candidates = Vec::new();
    while let Some(i) = pending.pop() {
        if nodes[i].tag == super::classify::Tag::Junk {
            candidates.push(i);
        } else {
            pending.extend(nodes[i].children.iter().copied());
        }
    }
    candidates.sort_unstable_by_key(|&i| std::cmp::Reverse(nodes[i].size));
    candidates
}

pub fn culprit(nodes: &[Node], mut i: usize) -> Option<usize> {
    loop {
        if ["node_modules", "target", ".git", ".venv"].contains(&nodes[i].name.as_str())
            || nodes[i].children.is_empty()
        {
            return Some(i);
        }
        let next = *nodes[i]
            .children
            .iter()
            .max_by_key(|&&child| nodes[child].size)?;
        if nodes[next].size <= nodes[i].size / 2 {
            return Some(i);
        }
        i = next;
    }
}
