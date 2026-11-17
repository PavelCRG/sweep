mod core;
mod plain;
mod ui;
mod update;

use clap::Parser;
use std::{
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Parser)]
#[command(name = "sweep", version, about = "Find what is using your disks")]
struct Args {
    #[arg(long, value_name = "PATH", num_args = 1.., action = clap::ArgAction::Append, conflicts_with = "demo", help = "Scan one or more directories instead of discovering mounted disks")]
    path: Vec<PathBuf>,
    #[arg(long, conflicts_with_all = ["plain", "json", "csv", "compare"])]
    dry_run: bool,
    #[arg(long)]
    demo: bool,
    #[arg(
        long,
        value_enum,
        value_name = "LANG",
        conflicts_with_all = ["plain", "json", "csv", "compare", "check_updates"],
        help = "TUI language (en or ru)"
    )]
    lang: Option<ui::lang::Language>,
    #[arg(long, conflicts_with_all = ["json", "csv", "compare"])]
    plain: bool,
    #[arg(long, conflicts_with_all = ["plain", "csv", "compare"])]
    json: bool,
    #[arg(long, conflicts_with_all = ["plain", "json", "compare"])]
    csv: bool,
    #[arg(long, value_name = "JSON_FILE", conflicts_with_all = ["plain", "json", "csv", "dry_run", "demo"])]
    compare: Option<PathBuf>,
    #[arg(long, conflicts_with_all = ["path", "dry_run", "demo", "plain", "json", "csv", "compare"])]
    check_updates: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.check_updates {
        match update::check(env!("CARGO_PKG_VERSION")) {
            Ok(update::UpdateStatus::Current) => {
                println!("sweep {} is up to date", env!("CARGO_PKG_VERSION"));
            }
            Ok(update::UpdateStatus::NoRelease) => {
                println!("No stable release has been published yet.");
            }
            Ok(update::UpdateStatus::Available { version, url }) => {
                println!("sweep {version} is available: {url}");
            }
            Err(error) => {
                eprintln!("Could not check for updates: {error}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal_flag = Arc::clone(&cancelled);
    ctrlc::set_handler(move || signal_flag.store(true, Ordering::Relaxed))?;
    if let Some(path) = &args.compare {
        let baseline: Vec<core::Node> = serde_json::from_reader(std::fs::File::open(path)?)?;
        let roots = if args.path.is_empty() {
            let mounts = core::volumes::mount_points();
            if mounts.is_empty() {
                vec![dirs_home()]
            } else {
                mounts
            }
        } else {
            args.path
        };
        let current = match scan_with_progress(&roots, &cancelled) {
            Ok(tree) => tree,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        plain::print_comparison(&current, &baseline);
        return Ok(());
    }
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
        match scan_with_progress(&roots, &cancelled) {
            Ok(tree) => tree,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    };
    if args.plain {
        plain::print_table(&tree);
    } else if args.csv {
        plain::print_csv(&tree);
    } else if args.json {
        plain::print_json(&tree)?;
    } else {
        ui::app::run(
            &mut tree,
            args.dry_run || args.demo,
            args.demo,
            args.lang.unwrap_or_default(),
            cancelled,
        )?;
    }
    Ok(())
}

fn scan_with_progress(
    roots: &[PathBuf],
    cancelled: &AtomicBool,
) -> std::io::Result<Vec<core::Node>> {
    eprint!("Scanning 0 items…");
    let _ = std::io::stderr().flush();
    let mut progress = core::scan::ScanProgress::default();
    let result = core::scan::scan_many_with_progress(roots, cancelled, |state| {
        progress = state;
        if state.visited % 500 == 0 {
            eprint!(
                "\rScanning {} items · {} skipped…",
                state.visited, state.skipped
            );
            let _ = std::io::stderr().flush();
        }
    });
    match &result {
        Ok(_) => eprintln!(
            "\rScanned {} items; {} entries skipped.          ",
            progress.visited, progress.skipped
        ),
        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
            eprintln!("\rScan cancelled.                    ");
        }
        Err(_) => {}
    }
    result
}

fn dirs_home() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::Args;
    use clap::Parser;

    #[test]
    fn cli_accepts_comparison_with_selected_paths() {
        assert!(
            Args::try_parse_from(["sweep", "--path", "src", "--compare", "before.json"]).is_ok()
        );
    }

    #[test]
    fn cli_rejects_conflicting_output_modes() {
        assert!(Args::try_parse_from(["sweep", "--dry-run", "--json"]).is_err());
        assert!(Args::try_parse_from(["sweep", "--plain", "--csv"]).is_err());
        assert!(Args::try_parse_from(["sweep", "--check-updates", "--path", "src"]).is_err());
        assert!(Args::try_parse_from(["sweep", "--plain", "--lang", "ru"]).is_err());
        assert!(Args::try_parse_from(["sweep", "--check-updates", "--lang", "ru"]).is_err());
    }

    #[test]
    fn cli_accepts_the_russian_interface_language() {
        assert!(Args::try_parse_from(["sweep", "--lang", "ru"]).is_ok());
    }
}
