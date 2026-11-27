//! rsdedup: A fast file deduplication tool.

mod action;
mod cache;
mod cli;
mod compare;
mod error;
mod grouper;
mod hasher;
mod output;
mod scanner;
mod types;

use anyhow::Result;
use cache::HashCache;
use clap::Parser;
use cli::{CacheAction, Cli, Commands, DedupAction};
use compare::find_duplicates;
use grouper::group_by_size;
use output::{print_groups, print_summary};
use scanner::{ScanOptions, scan};
use types::{DuplicateGroup, Summary};

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{} GB ({bytes} bytes)", output::scaled(bytes, GB))
    } else if bytes >= MB {
        format!("{} MB ({bytes} bytes)", output::scaled(bytes, MB))
    } else if bytes >= KB {
        format!("{} KB ({bytes} bytes)", output::scaled(bytes, KB))
    } else {
        format!("{bytes} bytes")
    }
}

fn format_timestamp(epoch_secs: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let time = UNIX_EPOCH + Duration::from_secs(epoch_secs);
    time.elapsed().map_or_else(
        |_| "in the future".to_string(),
        |ago| {
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
        },
    )
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
        eprintln!("found {total_files} files");
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

    let num_jobs = if cli.jobs == 0 {
        std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
    } else {
        cli.jobs
    };

    let duplicates = find_duplicates(size_groups, cli.compare, cli.hash, cache.as_ref(), num_jobs)?;

    Ok((duplicates, total_files))
}

fn finish_dedup(
    groups: &[DuplicateGroup],
    total_files: u64,
    cli: &Cli,
    action_taken: String,
    result: &action::ActionResult,
) -> i32 {
    let summary = Summary {
        files_scanned: total_files,
        duplicate_groups: groups.len() as u64,
        duplicate_files: groups.iter().map(|g| g.files.len() as u64 - 1).sum(),
        wasted_bytes: groups.iter().map(types::DuplicateGroup::wasted_bytes).sum(),
        action_taken,
        files_affected: result.files_affected,
        bytes_recovered: result.bytes_recovered,
    };
    print_summary(&summary, cli.output);
    if groups.is_empty() {
        error::EXIT_NO_DUPES
    } else {
        error::EXIT_DUPES_FOUND
    }
}

fn label(name: &str, dry_run: bool) -> String {
    if dry_run {
        format!("{name} (dry-run)")
    } else {
        name.to_string()
    }
}

fn run_dedup(action: &DedupAction, cli: &Cli) -> Result<i32> {
    let (path, dry_run) = match action {
        DedupAction::Report { path } => (path, false),
        DedupAction::Delete { path, dry_run, .. }
        | DedupAction::Hardlink { path, dry_run }
        | DedupAction::Symlink { path, dry_run } => (path, *dry_run),
    };
    let (groups, total_files) = run_pipeline(path, cli)?;
    if matches!(action, DedupAction::Report { .. }) || cli.verbose || dry_run {
        print_groups(&groups, cli.output);
    }
    let (name, result) = match action {
        DedupAction::Report { .. } => ("report", action::ActionResult::default()),
        DedupAction::Delete { keep, .. } => (
            "delete",
            action::delete_duplicates(&groups, *keep, dry_run, cli.verbose)?,
        ),
        DedupAction::Hardlink { .. } => (
            "hardlink",
            action::hardlink_duplicates(&groups, dry_run, cli.verbose)?,
        ),
        DedupAction::Symlink { .. } => (
            "symlink",
            action::symlink_duplicates(&groups, dry_run, cli.verbose)?,
        ),
    };
    Ok(finish_dedup(
        &groups,
        total_files,
        cli,
        label(name, dry_run),
        &result,
    ))
}

fn cache_scan(path: &std::path::Path, cli: &Cli) -> Result<i32> {
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

        let partial = if existing
            .as_ref()
            .and_then(|e| e.partial_hash.as_ref())
            .is_some()
        {
            None
        } else {
            hasher::hash_file(&entry.path, cli.hash, true).ok()
        };

        let full = if existing
            .as_ref()
            .and_then(|e| e.full_hash.as_ref())
            .is_some()
        {
            None
        } else {
            hasher::hash_file(&entry.path, cli.hash, false).ok()
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

    eprintln!("scanned {total_files} files: {hashed} hashed, {cached} already cached");
    if !cli.no_timing {
        let elapsed = start.elapsed();
        eprintln!("elapsed: {:.3}s", elapsed.as_secs_f64());
    }
    Ok(error::EXIT_SUCCESS)
}

fn cache_stats() -> Result<i32> {
    let cache = HashCache::open()?;
    let stats = cache.stats()?;
    println!("cache location:     {}", cache.path().display());
    println!("total entries:       {}", stats.entries);
    println!("database size:       {}", format_size(stats.db_size));
    println!(
        "total file size:     {}",
        format_size(stats.total_file_size)
    );
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
    Ok(error::EXIT_SUCCESS)
}

fn cache_list() -> Result<i32> {
    let cache = HashCache::open()?;
    println!("path\tsize\talgo\tpartial_hash\tfull_hash\tcached_at");
    for (path, entry) in cache.iter() {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            path,
            entry.size,
            entry.hash_algo,
            entry.partial_hash.as_deref().unwrap_or(""),
            entry.full_hash.as_deref().unwrap_or(""),
            entry.cached_at,
        );
    }
    Ok(error::EXIT_SUCCESS)
}

fn run_cache(action: &CacheAction, cli: &Cli) -> Result<i32> {
    match action {
        CacheAction::Scan { path } => cache_scan(path, cli),
        CacheAction::Clear => {
            let cache = HashCache::open()?;
            eprintln!("cache location: {}", cache.path().display());
            cache.clear()?;
            eprintln!("cache cleared");
            Ok(error::EXIT_SUCCESS)
        }
        CacheAction::Stats => cache_stats(),
        CacheAction::List => cache_list(),
        CacheAction::Prune => {
            let cache = HashCache::open()?;
            let removed = cache.prune()?;
            eprintln!("pruned {removed} stale entries");
            Ok(error::EXIT_SUCCESS)
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let Some(ref command) = cli.command else {
        Cli::print_short_help();
        error::exit_with(error::EXIT_SUCCESS);
    };

    let result: Result<i32> = match command {
        Commands::Dedup { action } => run_dedup(action, &cli),
        Commands::Cache { action } => run_cache(action, &cli),
        Commands::Version => {
            println!(
                "rsdedup {} by {}",
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_AUTHORS")
            );
            println!("GIT_DESCRIBE: {}", env!("GIT_DESCRIBE"));
            println!("GIT_SHA: {}", env!("GIT_SHA"));
            println!("GIT_BRANCH: {}", env!("GIT_BRANCH"));
            println!("GIT_DIRTY: {}", env!("GIT_DIRTY"));
            println!("RUSTC_SEMVER: {}", env!("RUSTC_SEMVER"));
            println!("RUST_EDITION: {}", env!("RUST_EDITION"));
            println!("BUILD_TIMESTAMP: {}", env!("BUILD_TIMESTAMP"));
            Ok(error::EXIT_SUCCESS)
        }
        Commands::Complete { shell } => {
            cli::generate_completions(*shell);
            Ok(error::EXIT_SUCCESS)
        }
    };

    match result {
        Ok(code) => error::exit_with(code),
        Err(err) => {
            eprintln!("error: {err:#}");
            error::exit_with(error::EXIT_ERROR);
        }
    }
}
