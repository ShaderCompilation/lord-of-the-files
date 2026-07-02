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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, parent_dir: &str) -> FileEntry {
        let path = format!("{parent_dir}/{id}");
        FileEntry {
            id: id.to_string(),
            path,
            parent_dir: parent_dir.to_string(),
            stem: id.to_string(),
            ext: String::new(),
            is_dir: false,
            size: 0,
            modified: None,
        }
    }

    fn row(id: &str, original: &str, new_name: &str, status: RowStatus) -> PreviewRow {
        PreviewRow {
            id: id.to_string(),
            original: original.to_string(),
            new_name: new_name.to_string(),
            status,
            message: None,
        }
    }

    #[test]
    fn validate_name_empty() {
        assert_eq!(validate_name(""), Some("Name is empty".to_string()));
        assert_eq!(validate_name("   "), Some("Name is empty".to_string()));
    }

    #[test]
    fn validate_name_each_invalid_char() {
        for c in ['<', '>', ':', '"', '/', '\\', '|', '?', '*'] {
            let name = format!("file{c}name.txt");
            let msg = validate_name(&name);
            assert!(
                msg.as_deref().is_some_and(|m| m.contains("invalid character")),
                "expected invalid-char message for {c:?}, got {msg:?}"
            );
        }
    }

    #[test]
    fn validate_name_control_character() {
        assert_eq!(
            validate_name("file\u{0007}.txt"),
            Some("Name contains a control character".to_string())
        );
    }

    #[test]
    fn validate_name_trailing_space() {
        assert_eq!(
            validate_name("file.txt "),
            Some("Name ends with a space or dot".to_string())
        );
    }

    #[test]
    fn validate_name_trailing_dot() {
        assert_eq!(
            validate_name("file."),
            Some("Name ends with a space or dot".to_string())
        );
    }

    #[test]
    fn validate_name_valid_returns_none() {
        assert_eq!(validate_name("file.txt"), None);
    }

    #[test]
    fn annotate_intra_batch_case_insensitive_collision() {
        let entries = vec![entry("a.txt", "/dir"), entry("b.txt", "/dir")];
        let mut rows = vec![
            row("a.txt", "a.txt", "Foo.txt", RowStatus::Changed),
            row("b.txt", "b.txt", "foo.txt", RowStatus::Changed),
        ];
        annotate(&entries, &mut rows);
        assert_eq!(rows[0].status, RowStatus::Conflict);
        assert_eq!(rows[1].status, RowStatus::Conflict);
    }

    #[test]
    fn annotate_unicode_case_insensitive_collision() {
        let entries = vec![entry("a.txt", "/dir"), entry("b.txt", "/dir")];
        let mut rows = vec![
            row("a.txt", "a.txt", "CAFÉ.txt", RowStatus::Changed),
            row("b.txt", "b.txt", "café.txt", RowStatus::Changed),
        ];
        annotate(&entries, &mut rows);
        assert_eq!(rows[0].status, RowStatus::Conflict);
        assert_eq!(rows[1].status, RowStatus::Conflict);
    }

    #[test]
    fn annotate_does_not_detect_nfc_nfd_normalization_collisions() {
        // Both render as "café.txt" but are byte-for-byte different: NFC uses a precomposed
        // é (U+00E9); NFD uses "e" + a combining acute accent (U+0301) — the form macOS's
        // APFS/HFS+ historically normalize filenames to. `annotate` does a plain lowercased
        // string compare with no Unicode normalization, so these are NOT flagged as colliding,
        // even though they could resolve to the same path on a normalization-folding
        // filesystem. Known, accepted v1 limitation — this test pins current behavior rather
        // than fixing it.
        let nfc_name = "caf\u{00E9}.txt";
        let nfd_name = "cafe\u{0301}.txt";
        let entries = vec![entry("a.txt", "/dir"), entry("b.txt", "/dir")];
        let mut rows = vec![
            row("a.txt", "a.txt", nfc_name, RowStatus::Changed),
            row("b.txt", "b.txt", nfd_name, RowStatus::Changed),
        ];
        annotate(&entries, &mut rows);
        assert_eq!(rows[0].status, RowStatus::Changed);
        assert_eq!(rows[1].status, RowStatus::Changed);
    }

    #[test]
    fn annotate_changed_collides_with_unchanged_sibling() {
        let entries = vec![entry("a.txt", "/dir"), entry("foo.txt", "/dir")];
        let mut rows = vec![
            row("a.txt", "a.txt", "foo.txt", RowStatus::Changed),
            row("foo.txt", "foo.txt", "foo.txt", RowStatus::Unchanged),
        ];
        annotate(&entries, &mut rows);
        assert_eq!(rows[0].status, RowStatus::Conflict);
        assert_eq!(rows[1].status, RowStatus::Unchanged);
    }

    #[test]
    fn annotate_preexisting_on_disk_collision() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_string_lossy().to_string();
        std::fs::write(dir.path().join("existing.txt"), "x").unwrap();

        let entries = vec![entry("a.txt", &dir_path)];
        let mut rows = vec![row("a.txt", "a.txt", "existing.txt", RowStatus::Changed)];
        annotate(&entries, &mut rows);
        assert_eq!(rows[0].status, RowStatus::Conflict);
        assert_eq!(
            rows[0].message.as_deref(),
            Some("A file with this name already exists")
        );
    }

    #[test]
    fn annotate_batch_orig_exemption_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_string_lossy().to_string();
        // b.txt exists on disk (it's entry B's original path/name) — B is moving away.
        std::fs::write(dir.path().join("b.txt"), "B").unwrap();

        let mut a = entry("a.txt", &dir_path);
        a.path = dir.path().join("a.txt").to_string_lossy().to_string();
        let mut b = entry("b.txt", &dir_path);
        b.path = dir.path().join("b.txt").to_string_lossy().to_string();
        let entries = vec![a, b];

        let mut rows = vec![
            row("a.txt", "a.txt", "b.txt", RowStatus::Changed),
            row("b.txt", "b.txt", "a.txt", RowStatus::Changed),
        ];
        annotate(&entries, &mut rows);
        // Both are renaming onto each other's original name (a swap); Pass C should not flag
        // either as a "pre-existing file" conflict since both original paths are in-batch.
        assert_eq!(rows[0].status, RowStatus::Changed);
        assert_eq!(rows[1].status, RowStatus::Changed);
    }
}
