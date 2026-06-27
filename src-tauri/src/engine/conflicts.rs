//! Validation and collision detection over computed preview rows.
//!
//! Defaults are cross-platform-safe: characters illegal on Windows are flagged even on
//! Linux, and collisions are detected case-insensitively (so a batch is safe to apply on a
//! case-insensitive filesystem too).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::types::{FileEntry, PreviewRow, RowStatus};

/// Characters that are invalid in a filename on at least one major OS.
const INVALID_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

fn validate_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Some("Name is empty".to_string());
    }
    if let Some(c) = name.chars().find(|c| INVALID_CHARS.contains(c) || (*c as u32) < 0x20) {
        if (c as u32) < 0x20 {
            return Some("Name contains a control character".to_string());
        }
        return Some(format!("Name contains an invalid character: {c}"));
    }
    if name.ends_with(' ') || name.ends_with('.') {
        return Some("Name ends with a space or dot".to_string());
    }
    None
}

/// Annotate rows in place with `Invalid` / `Conflict` status and messages.
pub fn annotate(entries: &[FileEntry], rows: &mut [PreviewRow]) {
    let parent: HashMap<&str, &str> = entries
        .iter()
        .map(|e| (e.id.as_str(), e.parent_dir.as_str()))
        .collect();
    // Lowercased original paths, so a file renamed onto a sibling that is itself moving
    // away is not flagged as pre-existing.
    let batch_orig: HashSet<String> = entries.iter().map(|e| e.path.to_lowercase()).collect();

    // Pass A: validity of changed rows.
    for row in rows.iter_mut() {
        if row.status != RowStatus::Changed {
            continue;
        }
        if let Some(msg) = validate_name(&row.new_name) {
            row.status = RowStatus::Invalid;
            row.message = Some(msg);
        }
    }

    // Pass B: collisions. Group every row by (parent dir, resulting name), case-insensitive.
    // If a group has more than one member and includes a changed row, the changed rows
    // collide.
    let mut groups: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (i, row) in rows.iter().enumerate() {
        if row.status == RowStatus::Invalid {
            continue;
        }
        let dir = parent.get(row.id.as_str()).copied().unwrap_or("");
        let key = (dir.to_lowercase(), row.new_name.to_lowercase());
        groups.entry(key).or_default().push(i);
    }
    for indices in groups.values() {
        if indices.len() < 2 {
            continue;
        }
        for &i in indices {
            if rows[i].status == RowStatus::Changed {
                rows[i].status = RowStatus::Conflict;
                rows[i].message = Some("Name collides with another file in the batch".to_string());
            }
        }
    }

    // Pass C: pre-existing files on disk (not part of this batch).
    for row in rows.iter_mut() {
        if row.status != RowStatus::Changed {
            continue;
        }
        let dir = parent.get(row.id.as_str()).copied().unwrap_or("");
        let target = Path::new(dir).join(&row.new_name);
        if target.exists() && !batch_orig.contains(&target.to_string_lossy().to_lowercase()) {
            row.status = RowStatus::Conflict;
            row.message = Some("A file with this name already exists".to_string());
        }
    }
}
