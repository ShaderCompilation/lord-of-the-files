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
    pub status: String, // "applied" | "undone"
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
            FOREIGN KEY (op_id) REFERENCES operations(id)
        );
        CREATE INDEX IF NOT EXISTS idx_renames_op ON renames(op_id);",
    )?;
    Ok(())
}

/// Move a batch of files via a temp staging phase. Returns the (from, to) pairs that
/// succeeded plus any failures.
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
    conn.execute(
        "INSERT INTO operations (id, created_at, summary, item_count, status)
         VALUES (?1, ?2, ?3, ?4, 'applied')",
        params![id, created_at, summary, pairs.len() as i64],
    )?;
    for (ord, (old, new)) in pairs.iter().enumerate() {
        conn.execute(
            "INSERT INTO renames (op_id, ord, old_path, new_path) VALUES (?1, ?2, ?3, ?4)",
            params![id, ord as i64, old, new],
        )?;
    }
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
    let operation_id = if done.is_empty() {
        None
    } else {
        record_operation(conn, &done).ok()
    };

    ApplyReport {
        operation_id,
        renamed: done.len(),
        failures,
    }
}

pub fn list_operations(conn: &Connection) -> rusqlite::Result<Vec<Operation>> {
    let mut stmt = conn.prepare(
        "SELECT id, created_at, summary, item_count, status FROM operations
         ORDER BY created_at DESC",
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
    conn.execute(
        "UPDATE operations SET status = 'undone' WHERE id = ?1",
        params![op_id],
    )?;
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
    conn.execute(
        "UPDATE operations SET status = 'applied' WHERE id = ?1",
        params![op_id],
    )?;
    Ok(UndoReport {
        reverted: done.len(),
        failures,
    })
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
}
