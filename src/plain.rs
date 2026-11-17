use crate::core::Node;

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
    for n in nodes.iter().filter(|n| n.size > 0) {
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
