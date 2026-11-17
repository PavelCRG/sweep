mod core;
mod plain;
mod ui;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sweep", version, about = "Find what is using your disks")]
struct Args {
    #[arg(long, value_name = "PATH", num_args = 1.., action = clap::ArgAction::Append, help = "Scan one or more paths instead of discovering mounted disks")]
    path: Vec<PathBuf>,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    demo: bool,
    #[arg(long, conflicts_with = "json")]
    plain: bool,
    #[arg(long)]
    json: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let roots = if args.path.is_empty() && !args.demo {
        let found = core::volumes::mount_points();
        if found.is_empty() {
            vec![dirs_home()]
        } else {
            found
        }
    } else if args.path.is_empty() {
        vec![dirs_home()]
    } else {
        args.path
    };
    let mut tree = if args.demo {
        core::demo::tree()
    } else {
        core::scan::scan_many(&roots)?
    };
    if args.plain {
        plain::print_table(&tree);
    } else if args.json {
        plain::print_json(&tree)?;
    } else {
        ui::app::run(&mut tree, args.dry_run || args.demo, args.demo)?;
    }
    Ok(())
}

fn dirs_home() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
