//! Persistent operation history with multi-level undo/redo, backed by SQLite.
//!
//! Renames are performed in two phases (source -> temp -> target) so that batches with
//! collisions, swaps, or case-only changes apply safely. Each successful batch is recorded
//! as one operation that can be undone or redone across app restarts.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// SQLite connection held in Tauri managed state.
pub struct HistoryDb(pub Mutex<Connection>);

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RenameItem {
    pub old_path: String,
    pub new_name: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub path: String,
    pub error: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReport {
    pub operation_id: Option<String>,
    pub renamed: usize,
    pub failures: Vec<Failure>,
    /// Set if the files were renamed on disk but the history record failed to save (e.g. a
    /// SQLite write error), meaning this batch will not be undoable from the History panel.
    pub history_error: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UndoReport {
    pub reverted: usize,
    pub failures: Vec<Failure>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub id: String,
    pub created_at: String,
    pub summary: String,
    pub item_count: i64,
    pub status: String, // "applied" | "undone" | "partial", derived from renames.status
}

/// Per-file detail of a past operation, including its persisted per-row status.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RenameEntry {
    pub old_path: String,
    pub new_path: String,
    pub status: String, // "applied" | "undone"
}

/// Result of a filesystem dry-run for one file in a prospective undo/redo.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum CheckStatus {
    Ok,
    Missing,
    WouldOverwrite,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileCheck {
    pub old_path: String,
    pub new_path: String,
    pub status: CheckStatus,
}

pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS operations (
            id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            summary TEXT NOT NULL,
            item_count INTEGER NOT NULL,
            status TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS renames (
            op_id TEXT NOT NULL,
            ord INTEGER NOT NULL,
            old_path TEXT NOT NULL,
            new_path TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'applied',
            FOREIGN KEY (op_id) REFERENCES operations(id)
        );
        CREATE INDEX IF NOT EXISTS idx_renames_op ON renames(op_id);",
    )?;
    migrate_add_rename_status(conn)?;
    Ok(())
}

/// Adds `renames.status` to installs from before per-file status tracking existed.
/// `CREATE TABLE IF NOT EXISTS` above is a no-op against pre-existing tables, so this
/// explicit check+backfill is required. Idempotent: a no-op once the column exists.
fn migrate_add_rename_status(conn: &Connection) -> rusqlite::Result<()> {
    let has_status = conn
        .prepare("PRAGMA table_info(renames)")?
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(Result::ok)
        .any(|name| name == "status");
    if !has_status {
        conn.execute_batch(
            "ALTER TABLE renames ADD COLUMN status TEXT NOT NULL DEFAULT 'applied';
             UPDATE renames SET status = (
                 SELECT operations.status FROM operations WHERE operations.id = renames.op_id
             );",
        )?;
    }
    Ok(())
}

/// Move a batch of files via a temp staging phase. Returns the (from, to) pairs that
/// succeeded plus any failures.
///
/// Note: the destination-exists check right before Phase 2's rename narrows but does not
/// eliminate the TOCTOU race against a file appearing at `to` between the check and the
/// rename — `std::fs::rename` has no portable no-clobber flag in stable std.
fn move_batch(moves: &[(String, String)]) -> (Vec<(String, String)>, Vec<Failure>) {
    let mut failures = Vec::new();
    // (temp, to, from)
    let mut staged: Vec<(String, String, String)> = Vec::new();

    for (from, to) in moves {
        if from == to {
            continue;
        }
        let parent = Path::new(to)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| Path::new(".").to_path_buf());
        let temp = parent.join(format!(".{}.lotf.tmp", Uuid::new_v4()));
        match std::fs::rename(from, &temp) {
            Ok(()) => staged.push((temp.to_string_lossy().to_string(), to.clone(), from.clone())),
            Err(e) => failures.push(Failure {
                path: from.clone(),
                error: e.to_string(),
            }),
        }
    }

    let mut done = Vec::new();
    for (temp, to, from) in staged {
        // By this point every in-batch source has already been staged away in Phase 1, so an
        // existing `to` here can only be a file outside this batch — never overwrite it.
        if Path::new(&to).exists() {
            let _ = std::fs::rename(&temp, &from);
            failures.push(Failure {
                path: from,
                error: format!("Destination already exists: {to}"),
            });
            continue;
        }
        match std::fs::rename(&temp, &to) {
            Ok(()) => done.push((from, to)),
            Err(e) => {
                // Best-effort rollback of the staging move.
                let _ = std::fs::rename(&temp, &from);
                failures.push(Failure {
                    path: from,
                    error: e.to_string(),
                });
            }
        }
    }

    (done, failures)
}

fn record_operation(conn: &Connection, pairs: &[(String, String)]) -> rusqlite::Result<String> {
    let id = Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    let summary = format!("Renamed {} file(s)", pairs.len());
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO operations (id, created_at, summary, item_count, status)
         VALUES (?1, ?2, ?3, ?4, 'applied')",
        params![id, created_at, summary, pairs.len() as i64],
    )?;
    for (ord, (old, new)) in pairs.iter().enumerate() {
        tx.execute(
            "INSERT INTO renames (op_id, ord, old_path, new_path) VALUES (?1, ?2, ?3, ?4)",
            params![id, ord as i64, old, new],
        )?;
    }
    tx.commit()?;
    Ok(id)
}

pub fn apply_rename(conn: &Connection, items: &[RenameItem]) -> ApplyReport {
    let moves: Vec<(String, String)> = items
        .iter()
        .map(|it| {
            let parent = Path::new(&it.old_path)
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| Path::new(".").to_path_buf());
            let to = parent.join(&it.new_name).to_string_lossy().to_string();
            (it.old_path.clone(), to)
        })
        .collect();

    let (done, failures) = move_batch(&moves);
    let (operation_id, history_error) = if done.is_empty() {
        (None, None)
    } else {
        match record_operation(conn, &done) {
            Ok(id) => (Some(id), None),
            Err(e) => {
                log::warn!("apply_rename: files renamed but history record failed: {e}");
                (None, Some(e.to_string()))
            }
        }
    };

    ApplyReport {
        operation_id,
        renamed: done.len(),
        failures,
        history_error,
    }
}

pub fn list_operations(conn: &Connection) -> rusqlite::Result<Vec<Operation>> {
    let mut stmt = conn.prepare(
        "SELECT o.id, o.created_at, o.summary, o.item_count,
            CASE
                WHEN COUNT(r.status) = 0 THEN o.status
                WHEN SUM(CASE WHEN r.status = 'applied' THEN 1 ELSE 0 END) = COUNT(r.status) THEN 'applied'
                WHEN SUM(CASE WHEN r.status = 'undone' THEN 1 ELSE 0 END) = COUNT(r.status) THEN 'undone'
                ELSE 'partial'
            END AS derived_status
         FROM operations o
         LEFT JOIN renames r ON r.op_id = o.id
         GROUP BY o.id
         ORDER BY o.created_at DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Operation {
                id: r.get(0)?,
                created_at: r.get(1)?,
                summary: r.get(2)?,
                item_count: r.get(3)?,
                status: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Per-file detail of an operation, for the History panel's expandable file list.
pub fn get_operation_files(conn: &Connection, op_id: &str) -> rusqlite::Result<Vec<RenameEntry>> {
    let mut stmt = conn.prepare(
        "SELECT old_path, new_path, status FROM renames WHERE op_id = ?1 ORDER BY ord",
    )?;
    let rows = stmt
        .query_map(params![op_id], |r| {
            Ok(RenameEntry {
                old_path: r.get(0)?,
                new_path: r.get(1)?,
                status: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Load the (old_path, new_path) pairs of an operation, ordered.
fn load_pairs(conn: &Connection, op_id: &str) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt =
        conn.prepare("SELECT old_path, new_path FROM renames WHERE op_id = ?1 ORDER BY ord")?;
    let rows = stmt
        .query_map(params![op_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Undo: move each file from its new path back to its old path.
pub fn undo_operation(conn: &Connection, op_id: &str) -> rusqlite::Result<UndoReport> {
    let pairs = load_pairs(conn, op_id)?;
    // Reverse order so later renames are undone first.
    let moves: Vec<(String, String)> = pairs
        .into_iter()
        .rev()
        .map(|(old, new)| (new, old))
        .collect();
    let (done, failures) = move_batch(&moves);
    // `done` entries are (from, to) = (new_path, old_path); only rows that actually
    // moved get marked undone. Rows in `failures` keep whatever status they had.
    for (from, to) in &done {
        conn.execute(
            "UPDATE renames SET status = 'undone' WHERE op_id = ?1 AND old_path = ?2 AND new_path = ?3",
            params![op_id, to, from],
        )?;
    }
    Ok(UndoReport {
        reverted: done.len(),
        failures,
    })
}

/// Redo: re-apply the operation (old -> new).
pub fn redo_operation(conn: &Connection, op_id: &str) -> rusqlite::Result<UndoReport> {
    let pairs = load_pairs(conn, op_id)?;
    let moves: Vec<(String, String)> = pairs;
    let (done, failures) = move_batch(&moves);
    for (from, to) in &done {
        conn.execute(
            "UPDATE renames SET status = 'applied' WHERE op_id = ?1 AND old_path = ?2 AND new_path = ?3",
            params![op_id, from, to],
        )?;
    }
    Ok(UndoReport {
        reverted: done.len(),
        failures,
    })
}

/// Dry-run: check whether each move in `moves` (already direction-adjusted) would
/// succeed, without touching the filesystem. A destination is only flagged as an
/// overwrite conflict if it's not itself a source elsewhere in this same batch —
/// intra-batch swaps/chains rely on move_batch's temp-staging to move safely.
fn dry_run(moves: &[(String, String)]) -> Vec<FileCheck> {
    use std::collections::HashSet;
    let sources: HashSet<&str> = moves.iter().map(|(from, _)| from.as_str()).collect();

    moves
        .iter()
        .map(|(from, to)| {
            let status = if from == to {
                CheckStatus::Ok
            } else if !Path::new(from).exists() {
                CheckStatus::Missing
            } else if Path::new(to).exists() && !sources.contains(to.as_str()) {
                CheckStatus::WouldOverwrite
            } else {
                CheckStatus::Ok
            };
            FileCheck {
                old_path: from.clone(),
                new_path: to.clone(),
                status,
            }
        })
        .collect()
}

/// Preview what `undo_operation` would do to the filesystem right now, without doing it.
pub fn preview_undo(conn: &Connection, op_id: &str) -> rusqlite::Result<Vec<FileCheck>> {
    let pairs = load_pairs(conn, op_id)?;
    let moves: Vec<(String, String)> = pairs
        .into_iter()
        .rev()
        .map(|(old, new)| (new, old))
        .collect();
    Ok(dry_run(&moves))
}

/// Preview what `redo_operation` would do to the filesystem right now, without doing it.
pub fn preview_redo(conn: &Connection, op_id: &str) -> rusqlite::Result<Vec<FileCheck>> {
    let pairs = load_pairs(conn, op_id)?;
    Ok(dry_run(&pairs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn apply_undo_redo_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        std::fs::write(&a, "hi").unwrap();
        let conn = mem_db();

        let items = vec![RenameItem {
            old_path: a.to_string_lossy().to_string(),
            new_name: "b.txt".to_string(),
        }];
        let report = apply_rename(&conn, &items);
        assert_eq!(report.renamed, 1);
        assert!(report.failures.is_empty());
        let op_id = report.operation_id.unwrap();
        assert!(dir.path().join("b.txt").exists());
        assert!(!a.exists());

        let u = undo_operation(&conn, &op_id).unwrap();
        assert_eq!(u.reverted, 1);
        assert!(a.exists());
        assert!(!dir.path().join("b.txt").exists());

        let r = redo_operation(&conn, &op_id).unwrap();
        assert_eq!(r.reverted, 1);
        assert!(dir.path().join("b.txt").exists());
    }

    #[test]
    fn swap_names_via_two_phase() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, "A").unwrap();
        std::fs::write(&b, "B").unwrap();
        let conn = mem_db();

        let items = vec![
            RenameItem {
                old_path: a.to_string_lossy().to_string(),
                new_name: "b.txt".to_string(),
            },
            RenameItem {
                old_path: b.to_string_lossy().to_string(),
                new_name: "a.txt".to_string(),
            },
        ];
        let report = apply_rename(&conn, &items);
        assert_eq!(report.renamed, 2);
        assert!(report.failures.is_empty());
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "B");
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "A");
    }

    #[test]
    fn migration_adds_status_column_to_existing_db() {
        let conn = Connection::open_in_memory().unwrap();
        // Simulate a pre-migration install: renames table has no status column.
        conn.execute_batch(
            "CREATE TABLE operations (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                summary TEXT NOT NULL,
                item_count INTEGER NOT NULL,
                status TEXT NOT NULL
            );
            CREATE TABLE renames (
                op_id TEXT NOT NULL,
                ord INTEGER NOT NULL,
                old_path TEXT NOT NULL,
                new_path TEXT NOT NULL,
                FOREIGN KEY (op_id) REFERENCES operations(id)
            );
            INSERT INTO operations (id, created_at, summary, item_count, status)
                VALUES ('op1', '2024-01-01T00:00:00Z', 'Renamed 1 file(s)', 1, 'undone');
            INSERT INTO renames (op_id, ord, old_path, new_path) VALUES ('op1', 0, '/a', '/b');",
        )
        .unwrap();

        init_schema(&conn).unwrap();

        let has_status = conn
            .prepare("PRAGMA table_info(renames)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(Result::ok)
            .any(|name| name == "status");
        assert!(has_status);

        let status: String = conn
            .query_row("SELECT status FROM renames WHERE op_id = 'op1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "undone");

        // Idempotent: running init_schema again doesn't error or re-ALTER.
        init_schema(&conn).unwrap();
    }

    #[test]
    fn partial_failure_marks_only_succeeded_rows() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        let c = dir.path().join("c.txt");
        std::fs::write(&a, "A").unwrap();
        std::fs::write(&b, "B").unwrap();
        std::fs::write(&c, "C").unwrap();
        let conn = mem_db();

        let items = vec![
            RenameItem {
                old_path: a.to_string_lossy().to_string(),
                new_name: "a2.txt".to_string(),
            },
            RenameItem {
                old_path: b.to_string_lossy().to_string(),
                new_name: "b2.txt".to_string(),
            },
            RenameItem {
                old_path: c.to_string_lossy().to_string(),
                new_name: "c2.txt".to_string(),
            },
        ];
        let report = apply_rename(&conn, &items);
        assert_eq!(report.renamed, 3);
        let op_id = report.operation_id.unwrap();

        // Simulate b2.txt being moved/deleted outside the app before undo.
        std::fs::remove_file(dir.path().join("b2.txt")).unwrap();

        let u = undo_operation(&conn, &op_id).unwrap();
        assert_eq!(u.reverted, 2);
        assert_eq!(u.failures.len(), 1);
        assert_eq!(
            u.failures[0].path,
            dir.path().join("b2.txt").to_string_lossy().to_string()
        );

        let files = get_operation_files(&conn, &op_id).unwrap();
        let status_for = |new_suffix: &str| {
            files
                .iter()
                .find(|f| f.new_path.ends_with(new_suffix))
                .unwrap()
                .status
                .clone()
        };
        assert_eq!(status_for("a2.txt"), "undone");
        assert_eq!(status_for("c2.txt"), "undone");
        assert_eq!(status_for("b2.txt"), "applied"); // untouched: kept its prior status
    }

    #[test]
    fn aggregate_status_reflects_mixed_rows() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, "A").unwrap();
        std::fs::write(&b, "B").unwrap();
        let conn = mem_db();

        let items = vec![
            RenameItem {
                old_path: a.to_string_lossy().to_string(),
                new_name: "a2.txt".to_string(),
            },
            RenameItem {
                old_path: b.to_string_lossy().to_string(),
                new_name: "b2.txt".to_string(),
            },
        ];
        let report = apply_rename(&conn, &items);
        let op_id = report.operation_id.unwrap();

        let ops = list_operations(&conn).unwrap();
        assert_eq!(
            ops.iter().find(|o| o.id == op_id).unwrap().status,
            "applied"
        );

        // Delete one renamed file, then undo: one row reverts, one fails => partial.
        std::fs::remove_file(dir.path().join("b2.txt")).unwrap();
        let u = undo_operation(&conn, &op_id).unwrap();
        assert_eq!(u.reverted, 1);
        assert_eq!(u.failures.len(), 1);

        let ops = list_operations(&conn).unwrap();
        assert_eq!(
            ops.iter().find(|o| o.id == op_id).unwrap().status,
            "partial"
        );

        // A separate, fully-undone batch aggregates to "undone".
        let c = dir.path().join("c.txt");
        std::fs::write(&c, "C").unwrap();
        let items2 = vec![RenameItem {
            old_path: c.to_string_lossy().to_string(),
            new_name: "c2.txt".to_string(),
        }];
        let report2 = apply_rename(&conn, &items2);
        let op_id2 = report2.operation_id.unwrap();
        undo_operation(&conn, &op_id2).unwrap();

        let ops = list_operations(&conn).unwrap();
        assert_eq!(
            ops.iter().find(|o| o.id == op_id2).unwrap().status,
            "undone"
        );
    }

    #[test]
    fn preview_undo_all_ok() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        std::fs::write(&a, "A").unwrap();
        let conn = mem_db();
        let items = vec![RenameItem {
            old_path: a.to_string_lossy().to_string(),
            new_name: "b.txt".to_string(),
        }];
        let report = apply_rename(&conn, &items);
        let op_id = report.operation_id.unwrap();

        let checks = preview_undo(&conn, &op_id).unwrap();
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].status, CheckStatus::Ok);
    }

    #[test]
    fn preview_undo_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        std::fs::write(&a, "A").unwrap();
        let conn = mem_db();
        let items = vec![RenameItem {
            old_path: a.to_string_lossy().to_string(),
            new_name: "b.txt".to_string(),
        }];
        let report = apply_rename(&conn, &items);
        let op_id = report.operation_id.unwrap();

        std::fs::remove_file(dir.path().join("b.txt")).unwrap();
        let checks = preview_undo(&conn, &op_id).unwrap();
        assert_eq!(checks[0].status, CheckStatus::Missing);
    }

    #[test]
    fn preview_undo_would_overwrite_external_collision() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        std::fs::write(&a, "A").unwrap();
        let conn = mem_db();
        let items = vec![RenameItem {
            old_path: a.to_string_lossy().to_string(),
            new_name: "b.txt".to_string(),
        }];
        let report = apply_rename(&conn, &items);
        let op_id = report.operation_id.unwrap();

        // Undo would move b.txt back to a.txt, but an unrelated a.txt now exists.
        std::fs::write(&a, "new content").unwrap();
        let checks = preview_undo(&conn, &op_id).unwrap();
        assert_eq!(checks[0].status, CheckStatus::WouldOverwrite);
    }

    #[test]
    fn preview_undo_swap_does_not_false_positive() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, "A").unwrap();
        std::fs::write(&b, "B").unwrap();
        let conn = mem_db();

        let items = vec![
            RenameItem {
                old_path: a.to_string_lossy().to_string(),
                new_name: "b.txt".to_string(),
            },
            RenameItem {
                old_path: b.to_string_lossy().to_string(),
                new_name: "a.txt".to_string(),
            },
        ];
        let report = apply_rename(&conn, &items);
        let op_id = report.operation_id.unwrap();

        let checks = preview_undo(&conn, &op_id).unwrap();
        assert_eq!(checks.len(), 2);
        assert!(checks.iter().all(|c| c.status == CheckStatus::Ok));
    }

    #[test]
    fn preview_redo_mirrors_direction() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        std::fs::write(&a, "A").unwrap();
        let conn = mem_db();
        let items = vec![RenameItem {
            old_path: a.to_string_lossy().to_string(),
            new_name: "b.txt".to_string(),
        }];
        let report = apply_rename(&conn, &items);
        let op_id = report.operation_id.unwrap();

        undo_operation(&conn, &op_id).unwrap();
        let checks = preview_redo(&conn, &op_id).unwrap();
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].status, CheckStatus::Ok);
    }

    #[test]
    fn apply_rename_does_not_overwrite_external_destination() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, "A").unwrap();
        std::fs::write(&b, "external").unwrap();
        let conn = mem_db();

        let items = vec![RenameItem {
            old_path: a.to_string_lossy().to_string(),
            new_name: "b.txt".to_string(),
        }];
        let report = apply_rename(&conn, &items);

        assert_eq!(report.renamed, 0);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].path, a.to_string_lossy().to_string());
        assert!(a.exists(), "source should be rolled back, not lost");
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "external");
    }

    #[test]
    fn undo_operation_does_not_overwrite_external_recreation_at_old_path() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, "A").unwrap();
        let conn = mem_db();

        let items = vec![RenameItem {
            old_path: a.to_string_lossy().to_string(),
            new_name: "b.txt".to_string(),
        }];
        let report = apply_rename(&conn, &items);
        let op_id = report.operation_id.unwrap();
        assert!(b.exists());

        // Externally recreate a.txt (the undo target) with different content.
        std::fs::write(&a, "external").unwrap();

        let u = undo_operation(&conn, &op_id).unwrap();
        assert_eq!(u.reverted, 0);
        assert_eq!(u.failures.len(), 1);
        assert!(b.exists(), "b.txt should be preserved, not lost");
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "external");
    }

    #[test]
    fn redo_operation_does_not_overwrite_external_recreation_at_new_path() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, "A").unwrap();
        let conn = mem_db();

        let items = vec![RenameItem {
            old_path: a.to_string_lossy().to_string(),
            new_name: "b.txt".to_string(),
        }];
        let report = apply_rename(&conn, &items);
        let op_id = report.operation_id.unwrap();
        undo_operation(&conn, &op_id).unwrap();
        assert!(a.exists());

        // Externally recreate b.txt (the redo target) with different content.
        std::fs::write(&b, "external").unwrap();

        let r = redo_operation(&conn, &op_id).unwrap();
        assert_eq!(r.reverted, 0);
        assert_eq!(r.failures.len(), 1);
        assert!(a.exists(), "a.txt should be preserved, not lost");
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "external");
    }

    #[test]
    fn apply_rename_logs_but_does_not_lose_files_on_history_failure() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        std::fs::write(&a, "A").unwrap();
        let conn = mem_db();
        // Force record_operation to fail while leaving the filesystem move itself intact.
        conn.execute_batch("DROP TABLE renames;").unwrap();

        let items = vec![RenameItem {
            old_path: a.to_string_lossy().to_string(),
            new_name: "b.txt".to_string(),
        }];
        let report = apply_rename(&conn, &items);

        assert_eq!(report.renamed, 1);
        assert!(report.failures.is_empty());
        assert!(dir.path().join("b.txt").exists());
        assert!(!a.exists());
        assert!(report.operation_id.is_none());
        assert!(report.history_error.is_some());
    }
}
