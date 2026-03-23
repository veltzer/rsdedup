use std::collections::HashMap;

use crate::types::FileEntry;

pub fn group_by_size(entries: Vec<FileEntry>) -> Vec<Vec<FileEntry>> {
    let mut map: HashMap<u64, Vec<FileEntry>> = HashMap::new();
    for entry in entries {
        map.entry(entry.size).or_default().push(entry);
    }
    map.into_values().filter(|group| group.len() > 1).collect()
}
