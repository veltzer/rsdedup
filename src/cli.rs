use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use std::io;
use std::path::PathBuf;

use crate::types::{CompareMethod, HashAlgo, KeepStrategy, OutputFormat};

#[derive(Parser)]
#[command(name = "rsdedup")]
#[command(version = concat!(env!("CARGO_PKG_VERSION")))]
#[command(about = "A fast file deduplication tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Comparison method
    #[arg(long, value_enum, default_value_t = CompareMethod::SizeHash, global = true, hide_short_help = true)]
    pub compare: CompareMethod,

    /// Hash algorithm
    #[arg(long, value_enum, default_value_t = HashAlgo::Sha256, global = true, hide_short_help = true)]
    pub hash: HashAlgo,

    /// Minimum file size to consider
    #[arg(long, global = true, hide_short_help = true)]
    pub min_size: Option<u64>,

    /// Maximum file size to consider
    #[arg(long, global = true, hide_short_help = true)]
    pub max_size: Option<u64>,

    /// Recurse into subdirectories
    #[arg(
        short,
        long,
        default_value_t = true,
        global = true,
        hide_short_help = true
    )]
    pub recursive: bool,

    /// Do not recurse into subdirectories
    #[arg(long, global = true, hide_short_help = true)]
    pub no_recursive: bool,

    /// Follow symbolic links
    #[arg(long, default_value_t = false, global = true, hide_short_help = true)]
    pub follow_symlinks: bool,

    /// Verbose output
    #[arg(
        short,
        long,
        default_value_t = false,
        global = true,
        hide_short_help = true
    )]
    pub verbose: bool,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true, hide_short_help = true)]
    pub output: OutputFormat,

    /// Number of parallel workers
    #[arg(short, long, default_value_t = num_cpus(), global = true, hide_short_help = true)]
    pub jobs: usize,

    /// Disable the hash cache
    #[arg(long, default_value_t = false, global = true, hide_short_help = true)]
    pub no_cache: bool,

    /// Disable timing output
    #[arg(long, default_value_t = false, global = true, hide_short_help = true)]
    pub no_timing: bool,

    /// Exclude files matching glob pattern (can be repeated)
    #[arg(long, global = true, hide_short_help = true)]
    pub exclude: Vec<String>,

    /// Only include files matching glob pattern (can be repeated)
    #[arg(long, global = true, hide_short_help = true)]
    pub include: Vec<String>,
}

impl Cli {
    pub fn print_short_help() {
        let cmd = Self::command();
        println!(
            "{} — {}\n",
            cmd.get_name(),
            cmd.get_about().unwrap_or_default()
        );
        println!("Commands:");
        for sub in cmd.get_subcommands() {
            if sub.get_name() == "help" {
                continue;
            }
            println!(
                "  {:12} {}",
                sub.get_name(),
                sub.get_about().unwrap_or_default()
            );
        }
        println!("\nRun 'rsdedup <command> --help' for more information on a command.");
    }
}

#[derive(Subcommand)]
pub enum Commands {
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
pub enum CacheAction {
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

pub fn generate_completions(shell: Shell) {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "rsdedup", &mut io::stdout());
}
