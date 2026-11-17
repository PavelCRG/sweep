use super::{
    classify::{self, Tag},
    Node,
};

pub fn clean(node: &Node, dry_run: bool) -> Result<String, Box<dyn std::error::Error>> {
    let canonical = if dry_run {
        std::fs::canonicalize(&node.path).unwrap_or_else(|_| node.path.clone())
    } else {
        std::fs::canonicalize(&node.path)?
    };
    let path = canonical.to_string_lossy();
    if node.tag == Tag::System
        || classify::is_system_area(&canonical)
        || path == "/"
        || (cfg!(windows) && path.len() <= 3)
    {
        return Err("System paths and their parent directories cannot be deleted".into());
    }
    if dry_run {
        return Ok(format!("Would move {} to Trash", node.path.display()));
    }
    trash::delete(&node.path)?;
    Ok(format!("Moved {} to Trash", node.name))
}
