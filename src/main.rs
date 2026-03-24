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
    command: Option<Commands>,

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

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB ({bytes} bytes)", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB ({bytes} bytes)", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB ({bytes} bytes)", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} bytes")
    }
}

fn format_timestamp(epoch_secs: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let time = UNIX_EPOCH + Duration::from_secs(epoch_secs);
    match time.elapsed() {
        Ok(ago) => {
            let secs = ago.as_secs();
            if secs < 60 {
                format!("{secs}s ago")
            } else if secs < 3600 {
                format!("{}m ago", secs / 60)
            } else if secs < 86400 {
                format!("{}h ago", secs / 3600)
            } else {
                format!("{}d ago", secs / 86400)
            }
        }
        Err(_) => "in the future".to_string(),
    }
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

    let command = match cli.command {
        Some(ref cmd) => cmd,
        None => {
            println!("rsdedup — A fast file deduplication tool\n");
            println!("Commands:");
            println!("  report       Find and report duplicate files (read-only)");
            println!("  delete       Delete duplicate files, keeping one copy per group");
            println!("  hardlink     Replace duplicates with hardlinks");
            println!("  symlink      Replace duplicates with symlinks");
            println!("  scan         Scan and populate the hash cache without dedup");
            println!("  cache        Manage the hash cache");
            println!("  version      Show version and build information");
            println!("  completions  Generate shell completions");
            println!("\nRun 'rsdedup <command> --help' for more information on a command.");
            error::exit_with(error::EXIT_NO_DUPES);
        }
    };

    let result: Result<i32> = (|| match command {
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

            // Hash every file (partial + full), using cache for files that haven't changed
            let mut cached = 0u64;
            let mut hashed = 0u64;
            for entry in &entries {
                let existing = cache
                    .as_ref()
                    .and_then(|c| c.lookup(&entry.path, cli.hash, &entry.metadata));

                let has_both = existing
                    .as_ref()
                    .is_some_and(|e| e.partial_hash.is_some() && e.full_hash.is_some());

                if has_both {
                    cached += 1;
                    continue;
                }

                let partial = if existing.as_ref().and_then(|e| e.partial_hash.as_ref()).is_some() {
                    None
                } else {
                    crate::hasher::hash_file(&entry.path, cli.hash, true).ok()
                };

                let full = if existing.as_ref().and_then(|e| e.full_hash.as_ref()).is_some() {
                    None
                } else {
                    crate::hasher::hash_file(&entry.path, cli.hash, false).ok()
                };

                if partial.is_some() || full.is_some() {
                    if let Some(ref c) = cache {
                        let _ = c.store(
                            &entry.path,
                            cli.hash,
                            &entry.metadata,
                            partial.as_deref(),
                            full.as_deref(),
                        );
                    }
                    hashed += 1;
                } else if existing.is_none() {
                    eprintln!("warning: failed to hash {}", entry.path.display());
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
                let stats = cache.stats()?;
                println!("cache location:     {}", cache.path().display());
                println!("total entries:       {}", stats.entries);
                println!("database size:       {}", format_size(stats.db_size));
                println!("total file size:     {}", format_size(stats.total_file_size));
                println!("with partial hash:   {}", stats.with_partial);
                println!("with full hash:      {}", stats.with_full);
                println!("stale (file gone):   {}", stats.stale);
                if let Some(oldest) = stats.oldest_timestamp {
                    println!("oldest entry:        {}", format_timestamp(oldest));
                }
                if let Some(newest) = stats.newest_timestamp {
                    println!("newest entry:        {}", format_timestamp(newest));
                }
                if !stats.algo_counts.is_empty() {
                    println!("hash algorithms:");
                    let mut algos: Vec<_> = stats.algo_counts.iter().collect();
                    algos.sort_by(|a, b| b.1.cmp(a.1));
                    for (algo, count) in algos {
                        println!("  {algo}: {count}");
                    }
                }
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
