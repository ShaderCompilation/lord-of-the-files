//! Full-chain test: scan -> compute_preview -> apply_rename -> undo, exercising the same
//! code paths the Tauri commands use (without the IPC layer).

use rusqlite::Connection;

use crate::engine;
use crate::fs_scan;
use crate::history::{self, RenameItem};
use crate::types::{Pipeline, RowStatus, Step, StepConfig};

fn mem_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    history::init_schema(&conn).unwrap();
    conn
}

#[test]
fn scan_preview_apply_undo_chain() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["IMG_1.txt", "IMG_2.txt", "report draft.md"] {
        std::fs::write(dir.path().join(name), "x").unwrap();
    }

    // Scan recursively.
    let entries = fs_scan::scan_paths(
        &[dir.path().to_string_lossy().to_string()],
        true,
        false,
    );
    assert_eq!(entries.len(), 3);

    // Pipeline: replace "IMG" with "Photo" in the stem.
    let pipeline = Pipeline {
        steps: vec![StepConfig {
            id: "s1".into(),
            enabled: true,
            scope: crate::types::Scope::Stem,
            step: Step::FindReplace {
                find: "IMG".into(),
                replace: "Photo".into(),
                case_sensitive: true,
                all_occurrences: true,
            },
        }],
    };

    let preview = engine::compute_preview(&entries, &pipeline);
    let changed: Vec<_> = preview
        .rows
        .iter()
        .filter(|r| r.status == RowStatus::Changed)
        .collect();
    assert_eq!(changed.len(), 2, "two IMG files should change");

    // Apply only the changed rows (id == path).
    let items: Vec<RenameItem> = changed
        .iter()
        .map(|r| RenameItem {
            old_path: r.id.clone(),
            new_name: r.new_name.clone(),
        })
        .collect();
    let conn = mem_db();
    let report = history::apply_rename(&conn, &items);
    assert_eq!(report.renamed, 2);
    assert!(report.failures.is_empty());
    assert!(dir.path().join("Photo_1.txt").exists());
    assert!(dir.path().join("Photo_2.txt").exists());
    assert!(!dir.path().join("IMG_1.txt").exists());
    assert!(dir.path().join("report draft.md").exists());

    // Undo restores the originals.
    let op_id = report.operation_id.unwrap();
    let undo = history::undo_operation(&conn, &op_id).unwrap();
    assert_eq!(undo.reverted, 2);
    assert!(dir.path().join("IMG_1.txt").exists());
    assert!(!dir.path().join("Photo_1.txt").exists());
}
