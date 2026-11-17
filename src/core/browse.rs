use super::Node;

pub fn sorted_children(nodes: &[Node], parent: usize) -> Vec<usize> {
    let mut out = nodes[parent].children.clone();
    out.sort_by_key(|&i| std::cmp::Reverse(nodes[i].size));
    out
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
