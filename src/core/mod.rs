pub mod browse;
pub mod categories;
pub mod classify;
pub mod clean;
pub mod demo;
pub mod scan;
pub mod volumes;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub files: u64,
    pub directory: bool,
    pub atime: Option<DateTime<Utc>>,
    pub mtime: Option<DateTime<Utc>>,
    pub children: Vec<usize>,
    pub tag: classify::Tag,
    pub reason: Option<String>,
    pub category: categories::Category,
}
