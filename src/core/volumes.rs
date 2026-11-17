use std::path::PathBuf;
use sysinfo::Disks;

pub fn mount_points() -> Vec<PathBuf> {
    let mut paths: Vec<_> = Disks::new_with_refreshed_list()
        .list()
        .iter()
        .filter(|disk| disk.total_space() > 0)
        .map(|disk| disk.mount_point().to_path_buf())
        .collect();
    paths.sort();
    paths.dedup();
    paths
}
