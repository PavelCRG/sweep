use crate::core::Node;
use std::collections::BTreeMap;

pub fn print_json(nodes: &[Node]) -> Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(nodes)?);
    Ok(())
}

pub fn print_table(nodes: &[Node]) {
    println!(
        "{:<40} {:>12} {:>10}  {:<8} CATEGORY",
        "PATH", "SIZE", "FILES", "TAG"
    );
    println!("{}", "─".repeat(94));
    for n in nodes {
        println!(
            "{:<40} {:>12} {:>10}  {:<8} {:?}",
            n.path
                .display()
                .to_string()
                .chars()
                .take(40)
                .collect::<String>(),
            human(n.size),
            n.files,
            format!("{:?}", n.tag),
            n.category
        );
    }
}

pub fn print_csv(nodes: &[Node]) {
    println!("path,name,size_bytes,files,directory,tag,reason,category,atime,mtime");
    for n in nodes {
        let atime = n.atime.as_ref().map(|d| d.to_rfc3339()).unwrap_or_default();
        let mtime = n.mtime.as_ref().map(|d| d.to_rfc3339()).unwrap_or_default();
        println!(
            "{},{},{},{},{},{},{},{},{},{}",
            csv_field(&n.path.display().to_string()),
            csv_field(&n.name),
            n.size,
            n.files,
            n.directory,
            csv_field(&format!("{:?}", n.tag)),
            csv_field(n.reason.as_deref().unwrap_or("")),
            csv_field(&format!("{:?}", n.category)),
            csv_field(&atime),
            csv_field(&mtime),
        );
    }
}

pub fn print_comparison(current: &[Node], baseline: &[Node]) {
    let changes = compare_sizes(
        current
            .iter()
            .map(|node| (node.path.to_string_lossy().into_owned(), node.size)),
        baseline
            .iter()
            .map(|node| (node.path.to_string_lossy().into_owned(), node.size)),
    );
    println!(
        "{:<64} {:>12} {:>12} {:>12}",
        "PATH", "BEFORE", "NOW", "CHANGE"
    );
    if changes.is_empty() {
        println!("No size changes.");
        return;
    }
    for (path, before, now, delta) in changes {
        println!(
            "{:<64} {:>12} {:>12} {:>12}",
            path.chars().take(64).collect::<String>(),
            human(before),
            human(now),
            human_delta(delta),
        );
    }
}

fn compare_sizes(
    current: impl IntoIterator<Item = (String, u64)>,
    baseline: impl IntoIterator<Item = (String, u64)>,
) -> Vec<(String, u64, u64, i128)> {
    let mut sizes = BTreeMap::new();
    for (path, size) in baseline {
        sizes.insert(path, (size, 0));
    }
    for (path, size) in current {
        sizes
            .entry(path)
            .and_modify(|row| row.1 = size)
            .or_insert((0, size));
    }
    let mut changes: Vec<_> = sizes
        .into_iter()
        .filter_map(|(path, (before, now))| {
            (before != now).then_some((path, before, now, i128::from(now) - i128::from(before)))
        })
        .collect();
    changes.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| a.0.cmp(&b.0)));
    changes
}

fn human_delta(bytes: i128) -> String {
    let sign = if bytes < 0 { "-" } else { "+" };
    format!(
        "{sign}{}",
        human(bytes.unsigned_abs().min(u64::MAX as u128) as u64)
    )
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

pub fn human(n: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024. && i < 4 {
        v /= 1024.;
        i += 1;
    }
    format!("{v:.1} {}", U[i])
}

#[cfg(test)]
mod tests {
    use super::{compare_sizes, csv_field, human_delta};

    #[test]
    fn csv_fields_escape_quotes_and_line_breaks() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,\"b\"\nnext"), "\"a,\"\"b\"\"\nnext\"");
    }

    #[test]
    fn comparison_delta_keeps_its_sign() {
        assert_eq!(human_delta(0), "+0.0 B");
        assert_eq!(human_delta(1_024), "+1.0 KB");
        assert_eq!(human_delta(-1_024), "-1.0 KB");
    }

    #[test]
    fn comparison_includes_added_and_removed_paths() {
        let changes = compare_sizes(
            [("/same".into(), 12), ("/new".into(), 9)],
            [("/same".into(), 10), ("/old".into(), 7)],
        );
        assert_eq!(
            changes,
            vec![
                ("/new".into(), 0, 9, 9),
                ("/same".into(), 10, 12, 2),
                ("/old".into(), 7, 0, -7),
            ]
        );
    }
}
