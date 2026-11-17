use super::{
    classify::{self, Tag},
    Node,
};

pub fn clean(node: &Node, dry_run: bool) -> Result<String, Box<dyn std::error::Error>> {
    let path = node.path.to_string_lossy();
    if node.tag == Tag::System
        || classify::is_system_area(&node.path)
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
