use crate::types::{DuplicateGroup, OutputFormat, Summary};

pub fn print_groups(groups: &[DuplicateGroup], format: OutputFormat) {
    match format {
        OutputFormat::Text => print_text(groups),
        OutputFormat::Json => print_json(groups),
    }
}

fn print_text(groups: &[DuplicateGroup]) {
    for (i, group) in groups.iter().enumerate() {
        println!(
            "Group {} — {} files, {} bytes each (hash: {}):",
            i + 1,
            group.files.len(),
            group.size,
            &group.hash[..group.hash.len().min(16)]
        );
        for file in &group.files {
            println!("  {}", file.path.display());
        }
        println!();
    }
}

fn print_json(groups: &[DuplicateGroup]) {
    #[derive(serde::Serialize)]
    struct JsonGroup {
        group: usize,
        size: u64,
        hash: String,
        files: Vec<String>,
    }

    let json_groups: Vec<JsonGroup> = groups
        .iter()
        .enumerate()
        .map(|(i, g)| JsonGroup {
            group: i + 1,
            size: g.size,
            hash: g.hash.clone(),
            files: g
                .files
                .iter()
                .map(|f| f.path.display().to_string())
                .collect(),
        })
        .collect();

    if let Ok(json) = serde_json::to_string_pretty(&json_groups) {
        println!("{json}");
    }
}

pub fn print_summary(summary: &Summary, format: OutputFormat) {
    match format {
        OutputFormat::Text => {
            println!("--- Summary ---");
            println!("Files scanned:    {}", summary.files_scanned);
            println!("Duplicate groups: {}", summary.duplicate_groups);
            println!("Duplicate files:  {}", summary.duplicate_files);
            println!("Wasted space:     {}", format_bytes(summary.wasted_bytes));
            if summary.files_affected > 0 || !summary.action_taken.is_empty() {
                println!("Action:           {}", summary.action_taken);
                println!("Files affected:   {}", summary.files_affected);
                println!(
                    "Space recovered:  {}",
                    format_bytes(summary.bytes_recovered)
                );
            }
        }
        OutputFormat::Json => {
            if let Ok(json) = serde_json::to_string_pretty(summary) {
                println!("{json}");
            }
        }
    }
}

/// `bytes / unit` rendered with two decimals (round half to even), using
/// integer arithmetic so no `u64` -> `f64` precision-losing cast is needed.
pub fn scaled(bytes: u64, unit: u64) -> String {
    let num = u128::from(bytes) * 100;
    let den = u128::from(unit);
    let mut q = num / den;
    let r = num % den;
    if r * 2 > den || (r * 2 == den && q % 2 == 1) {
        q += 1;
    }
    format!("{}.{:02}", q / 100, q % 100)
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{} TB", scaled(bytes, TB))
    } else if bytes >= GB {
        format!("{} GB", scaled(bytes, GB))
    } else if bytes >= MB {
        format!("{} MB", scaled(bytes, MB))
    } else if bytes >= KB {
        format!("{} KB", scaled(bytes, KB))
    } else {
        format!("{bytes} bytes")
    }
}
