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

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} bytes")
    }
}
