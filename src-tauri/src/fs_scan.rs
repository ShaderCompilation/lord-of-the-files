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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_non_recursive_only_top_level() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("top.txt"), "x").unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("nested.txt"), "x").unwrap();

        let entries = scan_paths(&[dir.path().to_string_lossy().to_string()], false, false);
        let names: Vec<_> = entries.iter().map(|e| e.stem.clone()).collect();
        assert_eq!(names, vec!["top"]);
    }

    #[test]
    fn scan_recursive_includes_nested_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("top.txt"), "x").unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("nested.txt"), "x").unwrap();

        let entries = scan_paths(&[dir.path().to_string_lossy().to_string()], true, false);
        let mut names: Vec<_> = entries.iter().map(|e| e.stem.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["nested", "top"]);
    }

    #[test]
    fn scan_include_dirs_true_adds_directory_entries() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("nested.txt"), "x").unwrap();

        let entries = scan_paths(&[dir.path().to_string_lossy().to_string()], true, true);
        // The walked "sub" directory and the top-level selected directory itself both count.
        let dirs: Vec<_> = entries.iter().filter(|e| e.is_dir).collect();
        assert_eq!(dirs.len(), 2);
        let sub_entry = dirs.iter().find(|e| e.stem == "sub").expect("sub dir entry present");
        assert_eq!(sub_entry.size, 0);
        assert_eq!(sub_entry.ext, "");
    }

    #[test]
    fn scan_include_dirs_false_excludes_directory_entries() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("nested.txt"), "x").unwrap();

        let entries = scan_paths(&[dir.path().to_string_lossy().to_string()], true, false);
        assert!(entries.iter().all(|e| !e.is_dir));
    }

    #[test]
    fn scan_deduplicates_same_path_passed_twice() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        let path = file.to_string_lossy().to_string();

        let entries = scan_paths(&[path.clone(), path], false, false);
        assert_eq!(entries.len(), 1);
    }
}
