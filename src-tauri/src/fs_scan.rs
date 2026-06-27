//! Turn a set of selected paths (files and/or folders) into a flat, de-duplicated list of
//! [`FileEntry`]s, optionally descending recursively into directories.

use std::collections::HashSet;
use std::path::Path;
use std::time::UNIX_EPOCH;

use walkdir::WalkDir;

use crate::types::FileEntry;

/// Split a filename into (stem, ext). A leading dot (dotfile) is part of the stem.
fn split_name(name: &str) -> (String, String) {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), ext.to_string()),
        _ => (name.to_string(), String::new()),
    }
}

fn entry_from_path(path: &Path) -> Option<FileEntry> {
    let meta = std::fs::metadata(path).ok()?;
    let is_dir = meta.is_dir();
    let name = path.file_name()?.to_string_lossy().to_string();
    let parent = path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let (stem, ext) = if is_dir {
        (name.clone(), String::new())
    } else {
        split_name(&name)
    };
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);

    Some(FileEntry {
        id: path.to_string_lossy().to_string(),
        path: path.to_string_lossy().to_string(),
        parent_dir: parent,
        stem,
        ext,
        is_dir,
        size: if is_dir { 0 } else { meta.len() },
        modified,
    })
}

/// Scan the given paths. Folders are expanded (recursively when `recursive`); directory
/// entries themselves are only included when `include_dirs` is set.
pub fn scan_paths(paths: &[String], recursive: bool, include_dirs: bool) -> Vec<FileEntry> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();

    let mut push = |path: &Path, entries: &mut Vec<FileEntry>| {
        let key = path.to_string_lossy().to_string();
        if seen.insert(key) {
            if let Some(e) = entry_from_path(path) {
                if e.is_dir && !include_dirs {
                    return;
                }
                entries.push(e);
            }
        }
    };

    for raw in paths {
        let path = Path::new(raw);
        if path.is_file() {
            push(path, &mut out);
        } else if path.is_dir() {
            let max_depth = if recursive { usize::MAX } else { 1 };
            for entry in WalkDir::new(path)
                .min_depth(1)
                .max_depth(max_depth)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                push(entry.path(), &mut out);
            }
            // Optionally include the selected directory itself.
            if include_dirs {
                push(path, &mut out);
            }
        }
    }

    out
}
