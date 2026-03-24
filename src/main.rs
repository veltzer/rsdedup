mod action;
mod cache;
mod compare;
mod error;
mod grouper;
mod hasher;
mod output;
mod scanner;
mod types;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use std::io;
use std::path::PathBuf;

use cache::HashCache;
use compare::find_duplicates;
use grouper::group_by_size;
use output::{print_groups, print_summary};
use scanner::{scan, ScanOptions};
use types::*;

#[derive(Parser)]
#[command(name = "rsdedup", version, about = "A fast file deduplication tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Comparison method
    #[arg(long, value_enum, default_value_t = CompareMethod::SizeHash, global = true)]
    compare: CompareMethod,

    /// Hash algorithm
    #[arg(long, value_enum, default_value_t = HashAlgo::Sha256, global = true)]
    hash: HashAlgo,

    /// Minimum file size to consider
    #[arg(long, global = true)]
    min_size: Option<u64>,

    /// Maximum file size to consider
    #[arg(long, global = true)]
    max_size: Option<u64>,

    /// Recurse into subdirectories
    #[arg(short, long, default_value_t = true, global = true)]
    recursive: bool,

    /// Do not recurse into subdirectories
    #[arg(long, global = true)]
    no_recursive: bool,

    /// Follow symbolic links
    #[arg(long, default_value_t = false, global = true)]
    follow_symlinks: bool,

    /// Verbose output
    #[arg(short, long, default_value_t = false, global = true)]
    verbose: bool,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    output: OutputFormat,

    /// Number of parallel workers
    #[arg(short, long, default_value_t = num_cpus(), global = true)]
    jobs: usize,

    /// Disable the hash cache
    #[arg(long, default_value_t = false, global = true)]
    no_cache: bool,

    /// Disable timing output
    #[arg(long, default_value_t = false, global = true)]
    no_timing: bool,

    /// Exclude files matching glob pattern (can be repeated)
    #[arg(long, global = true)]
    exclude: Vec<String>,

    /// Only include files matching glob pattern (can be repeated)
    #[arg(long, global = true)]
    include: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Find and report duplicate files (read-only)
    Report {
        /// Directory to scan
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Delete duplicate files, keeping one copy per group
    Delete {
        /// Directory to scan
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Which file to keep
        #[arg(long, value_enum, default_value_t = KeepStrategy::First)]
        keep: KeepStrategy,
        /// Show what would be done without making changes
        #[arg(short = 'n', long, default_value_t = false)]
        dry_run: bool,
    },
    /// Replace duplicates with hardlinks
    Hardlink {
        /// Directory to scan
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Show what would be done without making changes
        #[arg(short = 'n', long, default_value_t = false)]
        dry_run: bool,
    },
    /// Replace duplicates with symlinks
    Symlink {
        /// Directory to scan
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Show what would be done without making changes
        #[arg(short = 'n', long, default_value_t = false)]
        dry_run: bool,
    },
    /// Scan and populate the hash cache without dedup
    Scan {
        /// Directory to scan
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Manage the hash cache
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
    /// Show version and build information
    Version,
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Subcommand)]
enum CacheAction {
    /// Clear the hash cache
    Clear,
    /// Show cache statistics
    Stats,
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

fn run_pipeline(path: &std::path::Path, cli: &Cli) -> Result<(Vec<DuplicateGroup>, u64)> {
    let scan_opts = ScanOptions {
        recursive: cli.recursive && !cli.no_recursive,
        follow_symlinks: cli.follow_symlinks,
        min_size: cli.min_size,
        max_size: cli.max_size,
        include: cli.include.clone(),
        exclude: cli.exclude.clone(),
    };

    if cli.verbose {
        eprintln!("scanning {}...", path.display());
    }

    let entries = scan(path, &scan_opts)?;
    let total_files = entries.len() as u64;

    if cli.verbose {
        eprintln!("found {} files", total_files);
    }

    let size_groups = group_by_size(entries);

    if cli.verbose {
        eprintln!(
            "{} size groups with potential duplicates",
            size_groups.len()
        );
    }

    let cache = if cli.no_cache {
        None
    } else {
        HashCache::open().ok()
    };

    let duplicates = find_duplicates(size_groups, cli.compare, cli.hash, cache.as_ref(), cli.jobs)?;

    if let Some(ref c) = cache {
        let _ = c.flush();
    }

    Ok((duplicates, total_files))
}

fn main() {
    let cli = Cli::parse();

    let result: Result<i32> = (|| match &cli.command {
        Commands::Report { path } => {
            let (groups, total_files) = run_pipeline(path, &cli)?;
            let has_dupes = !groups.is_empty();
            print_groups(&groups, cli.output);
            let summary = Summary {
                files_scanned: total_files,
                duplicate_groups: groups.len() as u64,
                duplicate_files: groups.iter().map(|g| g.files.len() as u64 - 1).sum(),
                wasted_bytes: groups.iter().map(|g| g.wasted_bytes()).sum(),
                action_taken: "report".to_string(),
                files_affected: 0,
                bytes_recovered: 0,
            };
            print_summary(&summary, cli.output);
            Ok(if has_dupes {
                error::EXIT_DUPES_FOUND
            } else {
                error::EXIT_NO_DUPES
            })
        }
        Commands::Delete {
            path,
            keep,
            dry_run,
        } => {
            let (groups, total_files) = run_pipeline(path, &cli)?;
            let has_dupes = !groups.is_empty();
            if cli.verbose || *dry_run {
                print_groups(&groups, cli.output);
            }
            let result = action::delete_duplicates(&groups, *keep, *dry_run, cli.verbose)?;
            let summary = Summary {
                files_scanned: total_files,
                duplicate_groups: groups.len() as u64,
                duplicate_files: groups.iter().map(|g| g.files.len() as u64 - 1).sum(),
                wasted_bytes: groups.iter().map(|g| g.wasted_bytes()).sum(),
                action_taken: if *dry_run {
                    "delete (dry-run)".to_string()
                } else {
                    "delete".to_string()
                },
                files_affected: result.files_affected,
                bytes_recovered: result.bytes_recovered,
            };
            print_summary(&summary, cli.output);
            Ok(if has_dupes {
                error::EXIT_DUPES_FOUND
            } else {
                error::EXIT_NO_DUPES
            })
        }
        Commands::Hardlink { path, dry_run } => {
            let (groups, total_files) = run_pipeline(path, &cli)?;
            let has_dupes = !groups.is_empty();
            if cli.verbose || *dry_run {
                print_groups(&groups, cli.output);
            }
            let result = action::hardlink_duplicates(&groups, *dry_run, cli.verbose)?;
            let summary = Summary {
                files_scanned: total_files,
                duplicate_groups: groups.len() as u64,
                duplicate_files: groups.iter().map(|g| g.files.len() as u64 - 1).sum(),
                wasted_bytes: groups.iter().map(|g| g.wasted_bytes()).sum(),
                action_taken: if *dry_run {
                    "hardlink (dry-run)".to_string()
                } else {
                    "hardlink".to_string()
                },
                files_affected: result.files_affected,
                bytes_recovered: result.bytes_recovered,
            };
            print_summary(&summary, cli.output);
            Ok(if has_dupes {
                error::EXIT_DUPES_FOUND
            } else {
                error::EXIT_NO_DUPES
            })
        }
        Commands::Symlink { path, dry_run } => {
            let (groups, total_files) = run_pipeline(path, &cli)?;
            let has_dupes = !groups.is_empty();
            if cli.verbose || *dry_run {
                print_groups(&groups, cli.output);
            }
            let result = action::symlink_duplicates(&groups, *dry_run, cli.verbose)?;
            let summary = Summary {
                files_scanned: total_files,
                duplicate_groups: groups.len() as u64,
                duplicate_files: groups.iter().map(|g| g.files.len() as u64 - 1).sum(),
                wasted_bytes: groups.iter().map(|g| g.wasted_bytes()).sum(),
                action_taken: if *dry_run {
                    "symlink (dry-run)".to_string()
                } else {
                    "symlink".to_string()
                },
                files_affected: result.files_affected,
                bytes_recovered: result.bytes_recovered,
            };
            print_summary(&summary, cli.output);
            Ok(if has_dupes {
                error::EXIT_DUPES_FOUND
            } else {
                error::EXIT_NO_DUPES
            })
        }
        Commands::Scan { path } => {
            let start = std::time::Instant::now();
            let scan_opts = ScanOptions {
                recursive: cli.recursive && !cli.no_recursive,
                follow_symlinks: cli.follow_symlinks,
                min_size: cli.min_size,
                max_size: cli.max_size,
                include: cli.include.clone(),
                exclude: cli.exclude.clone(),
            };

            if cli.verbose {
                eprintln!("scanning {}...", path.display());
            }

            let entries = scan(path, &scan_opts)?;
            let total_files = entries.len() as u64;

            let cache = if cli.no_cache {
                None
            } else {
                HashCache::open().ok()
            };

            if let Some(ref c) = cache {
                eprintln!("cache location: {}", c.path().display());
            }

            // Hash every file, using cache for files that haven't changed
            let mut cached = 0u64;
            let mut hashed = 0u64;
            for entry in &entries {
                let already_cached = cache
                    .as_ref()
                    .and_then(|c| c.lookup(&entry.path, cli.hash, &entry.metadata))
                    .and_then(|e| e.full_hash)
                    .is_some();

                if already_cached {
                    cached += 1;
                    continue;
                }

                match crate::hasher::hash_file(&entry.path, cli.hash, false) {
                    Ok(hash) => {
                        if let Some(ref c) = cache {
                            let _ = c.store(
                                &entry.path,
                                cli.hash,
                                &entry.metadata,
                                None,
                                Some(&hash),
                            );
                        }
                        hashed += 1;
                    }
                    Err(err) => {
                        eprintln!("warning: failed to hash {}: {err}", entry.path.display());
                    }
                }
            }

            if let Some(ref c) = cache {
                let _ = c.flush();
            }

            eprintln!(
                "scanned {total_files} files: {hashed} hashed, {cached} already cached"
            );
            if !cli.no_timing {
                let elapsed = start.elapsed();
                eprintln!("elapsed: {:.3}s", elapsed.as_secs_f64());
            }
            Ok(error::EXIT_NO_DUPES)
        }
        Commands::Cache { action } => match action {
            CacheAction::Clear => {
                let cache = HashCache::open()?;
                eprintln!("cache location: {}", cache.path().display());
                cache.clear()?;
                eprintln!("cache cleared");
                Ok(error::EXIT_NO_DUPES)
            }
            CacheAction::Stats => {
                let cache = HashCache::open()?;
                let (count, size) = cache.stats()?;
                println!("cache location: {}", cache.path().display());
                println!("cache entries: {count}");
                println!("cache size on disk: {size} bytes");
                Ok(error::EXIT_NO_DUPES)
            }
        },
        Commands::Version => {
            println!("rsdedup {} by {}", env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_AUTHORS"));
            println!("GIT_DESCRIBE: {}", env!("GIT_DESCRIBE"));
            println!("GIT_SHA: {}", env!("GIT_SHA"));
            println!("GIT_BRANCH: {}", env!("GIT_BRANCH"));
            println!("GIT_DIRTY: {}", env!("GIT_DIRTY"));
            println!("RUSTC_SEMVER: {}", env!("RUSTC_SEMVER"));
            println!("RUST_EDITION: {}", env!("RUST_EDITION"));
            println!("BUILD_TIMESTAMP: {}", env!("BUILD_TIMESTAMP"));
            Ok(error::EXIT_NO_DUPES)
        }
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            generate(*shell, &mut cmd, "rsdedup", &mut io::stdout());
            Ok(error::EXIT_NO_DUPES)
        }
    })();

    match result {
        Ok(code) => error::exit_with(code),
        Err(err) => {
            eprintln!("error: {err:#}");
            error::exit_with(error::EXIT_ERROR);
        }
    }
}
