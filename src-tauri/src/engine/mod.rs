//! The rename engine: turns a [`Pipeline`] of steps into new names for a batch of files.
//!
//! This is the single source of truth — both the live preview and the actual apply derive
//! their new names from [`compute_preview`], so they can never diverge.

pub mod conflicts;
pub mod steps;

use std::collections::HashMap;

use regex::RegexBuilder;

use crate::types::*;

/// A filename split into stem + extension, mutated as steps run.
#[derive(Clone, Debug)]
pub struct NameParts {
    pub stem: String,
    pub ext: String,
}

impl NameParts {
    pub fn assemble(&self) -> String {
        if self.ext.is_empty() {
            self.stem.clone()
        } else {
            format!("{}.{}", self.stem, self.ext)
        }
    }
}

/// Read the portion of the name a step targets.
fn get_target(p: &NameParts, scope: Scope) -> String {
    match scope {
        Scope::Stem => p.stem.clone(),
        Scope::Ext => p.ext.clone(),
        Scope::Full => p.assemble(),
    }
}

/// Write back the transformed portion of the name.
fn set_target(p: &mut NameParts, scope: Scope, value: String) {
    match scope {
        Scope::Stem => p.stem = value,
        Scope::Ext => p.ext = value,
        Scope::Full => match value.rsplit_once('.') {
            // A leading dot (dotfile) is part of the stem, not an extension.
            Some((s, e)) if !s.is_empty() => {
                p.stem = s.to_string();
                p.ext = e.to_string();
            }
            _ => {
                p.stem = value;
                p.ext = String::new();
            }
        },
    }
}

/// Per-file context that some steps need (counter numbering, AI lookups).
struct StepCtx<'a> {
    file_id: &'a str,
    index: usize,
    dir_index: usize,
}

/// A step pre-processed for execution (regex compiled, AI results indexed).
struct Prepared<'a> {
    cfg: &'a StepConfig,
    regex: Option<regex::Regex>,
    ai_map: Option<HashMap<String, String>>,
}

/// Compile/prepare the enabled steps once. Returns prepared steps and any step errors
/// (e.g. an invalid regex). Steps that fail to prepare are skipped, not fatal.
fn prepare<'a>(pipeline: &'a Pipeline) -> (Vec<Prepared<'a>>, Vec<StepError>) {
    let mut prepared = Vec::new();
    let mut errors = Vec::new();

    for cfg in &pipeline.steps {
        if !cfg.enabled {
            continue;
        }
        match &cfg.step {
            Step::Regex {
                pattern,
                ignore_case,
                dotall,
                multiline,
                ..
            } => {
                match RegexBuilder::new(pattern)
                    .case_insensitive(*ignore_case)
                    .dot_matches_new_line(*dotall)
                    .multi_line(*multiline)
                    .build()
                {
                    Ok(re) => prepared.push(Prepared {
                        cfg,
                        regex: Some(re),
                        ai_map: None,
                    }),
                    Err(e) => errors.push(StepError {
                        step_id: cfg.id.clone(),
                        message: format!("Invalid regex: {e}"),
                    }),
                }
            }
            Step::Ai { results, .. } => {
                let map = results.as_ref().map(|items| {
                    items
                        .iter()
                        .map(|i| (i.id.clone(), i.new_name.clone()))
                        .collect::<HashMap<_, _>>()
                });
                prepared.push(Prepared {
                    cfg,
                    regex: None,
                    ai_map: map,
                });
            }
            _ => prepared.push(Prepared {
                cfg,
                regex: None,
                ai_map: None,
            }),
        }
    }

    (prepared, errors)
}

/// Apply one prepared step to a name in place.
fn apply_step(prep: &Prepared, parts: &mut NameParts, ctx: &StepCtx) {
    let scope = prep.cfg.scope;
    let target = get_target(parts, scope);

    let result = match &prep.cfg.step {
        Step::FindReplace {
            find,
            replace,
            case_sensitive,
            all_occurrences,
        } => steps::find_replace(&target, find, replace, *case_sensitive, *all_occurrences),
        Step::Regex { replacement, .. } => match &prep.regex {
            Some(re) => steps::regex_replace(&target, re, replacement),
            None => target,
        },
        Step::ChangeCase { mode } => steps::change_case(&target, *mode),
        Step::Insert {
            text,
            position,
            index,
        } => steps::insert(&target, text, *position, *index),
        Step::Remove { from, count, index } => steps::remove(&target, *from, *count, *index),
        Step::CleanUp {
            trim,
            collapse_whitespace,
            spaces_to,
            strip_diacritics,
        } => steps::clean_up(
            &target,
            *trim,
            *collapse_whitespace,
            spaces_to.as_deref(),
            *strip_diacritics,
        ),
        Step::Counter {
            start,
            step,
            padding,
            separator,
            position,
            reset_per_directory,
        } => {
            let n = if *reset_per_directory {
                ctx.dir_index
            } else {
                ctx.index
            };
            let value = *start + (n as i64) * *step;
            steps::counter_affix(&target, value, *padding, separator, *position)
        }
        Step::Ai { .. } => match &prep.ai_map {
            Some(map) => map.get(ctx.file_id).cloned().unwrap_or(target),
            None => target,
        },
    };

    set_target(parts, scope, result);
}

/// Run the pipeline over every entry and produce preview rows (without conflict checks).
fn run_pipeline(entries: &[FileEntry], pipeline: &Pipeline) -> (Vec<PreviewRow>, Vec<StepError>) {
    let (prepared, errors) = prepare(pipeline);
    let mut dir_counters: HashMap<&str, usize> = HashMap::new();
    let mut rows = Vec::with_capacity(entries.len());

    for (index, entry) in entries.iter().enumerate() {
        let dir_index = {
            let c = dir_counters.entry(entry.parent_dir.as_str()).or_insert(0);
            let v = *c;
            *c += 1;
            v
        };

        let mut parts = NameParts {
            stem: entry.stem.clone(),
            ext: entry.ext.clone(),
        };
        let ctx = StepCtx {
            file_id: &entry.id,
            index,
            dir_index,
        };
        for prep in &prepared {
            apply_step(prep, &mut parts, &ctx);
        }

        let original = NameParts {
            stem: entry.stem.clone(),
            ext: entry.ext.clone(),
        }
        .assemble();
        let new_name = parts.assemble();
        let status = if new_name == original {
            RowStatus::Unchanged
        } else {
            RowStatus::Changed
        };

        rows.push(PreviewRow {
            id: entry.id.clone(),
            original,
            new_name,
            status,
            message: None,
        });
    }

    (rows, errors)
}

/// Full preview: run the pipeline then annotate rows with conflict/validation status.
pub fn compute_preview(entries: &[FileEntry], pipeline: &Pipeline) -> PreviewResult {
    let (mut rows, step_errors) = run_pipeline(entries, pipeline);
    conflicts::annotate(entries, &mut rows);
    PreviewResult { rows, step_errors }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, parent_dir: &str, stem: &str, ext: &str) -> FileEntry {
        FileEntry {
            id: id.to_string(),
            path: format!("{parent_dir}/{stem}.{ext}"),
            parent_dir: parent_dir.to_string(),
            stem: stem.to_string(),
            ext: ext.to_string(),
            is_dir: false,
            size: 0,
            modified: None,
        }
    }

    fn step(scope: Scope, step: Step) -> StepConfig {
        StepConfig {
            id: "s".to_string(),
            enabled: true,
            scope,
            step,
        }
    }

    #[test]
    fn full_scope_multidot_filename_round_trip() {
        let entries = vec![entry("a", "/dir", "archive.tar", "gz")];
        let pipeline = Pipeline {
            steps: vec![step(
                Scope::Full,
                Step::FindReplace {
                    find: "tar".to_string(),
                    replace: "zip".to_string(),
                    case_sensitive: true,
                    all_occurrences: true,
                },
            )],
        };
        let (rows, _) = run_pipeline(&entries, &pipeline);
        assert_eq!(rows[0].original, "archive.tar.gz");
        assert_eq!(rows[0].new_name, "archive.zip.gz");
    }

    #[test]
    fn unicode_stem_survives_find_replace_and_case_change() {
        let entries = vec![entry("a", "/dir", "café résumé 日本語", "txt")];
        let pipeline = Pipeline {
            steps: vec![
                step(
                    Scope::Stem,
                    Step::FindReplace {
                        find: "résumé".to_string(),
                        replace: "CV".to_string(),
                        case_sensitive: true,
                        all_occurrences: true,
                    },
                ),
                step(Scope::Stem, Step::ChangeCase { mode: CaseMode::Upper }),
            ],
        };
        let (rows, _) = run_pipeline(&entries, &pipeline);
        assert_eq!(rows[0].new_name, "CAFÉ CV 日本語.txt");
    }

    #[test]
    fn counter_reset_per_directory_across_multiple_dirs() {
        let entries = vec![
            entry("a1", "/a", "file", "txt"),
            entry("b1", "/b", "file", "txt"),
            entry("a2", "/a", "file", "txt"),
            entry("b2", "/b", "file", "txt"),
        ];
        let pipeline = Pipeline {
            steps: vec![step(
                Scope::Stem,
                Step::Counter {
                    start: 1,
                    step: 1,
                    padding: 0,
                    separator: "_".to_string(),
                    position: AffixPosition::Suffix,
                    reset_per_directory: true,
                },
            )],
        };
        let (rows, _) = run_pipeline(&entries, &pipeline);
        let name_for = |id: &str| rows.iter().find(|r| r.id == id).unwrap().new_name.clone();
        assert_eq!(name_for("a1"), "file_1.txt");
        assert_eq!(name_for("a2"), "file_2.txt");
        assert_eq!(name_for("b1"), "file_1.txt");
        assert_eq!(name_for("b2"), "file_2.txt");
    }

    #[test]
    fn counter_no_reset_uses_global_index() {
        let entries = vec![
            entry("a1", "/a", "file", "txt"),
            entry("b1", "/b", "file", "txt"),
            entry("a2", "/a", "file", "txt"),
            entry("b2", "/b", "file", "txt"),
        ];
        let pipeline = Pipeline {
            steps: vec![step(
                Scope::Stem,
                Step::Counter {
                    start: 1,
                    step: 1,
                    padding: 0,
                    separator: "_".to_string(),
                    position: AffixPosition::Suffix,
                    reset_per_directory: false,
                },
            )],
        };
        let (rows, _) = run_pipeline(&entries, &pipeline);
        let name_for = |id: &str| rows.iter().find(|r| r.id == id).unwrap().new_name.clone();
        assert_eq!(name_for("a1"), "file_1.txt");
        assert_eq!(name_for("b1"), "file_2.txt");
        assert_eq!(name_for("a2"), "file_3.txt");
        assert_eq!(name_for("b2"), "file_4.txt");
    }

    #[test]
    fn multi_step_pipeline_applies_in_order() {
        let entries = vec![entry("a", "/dir", "foo", "txt")];
        let pipeline = Pipeline {
            steps: vec![
                step(
                    Scope::Stem,
                    Step::FindReplace {
                        find: "foo".to_string(),
                        replace: "bar".to_string(),
                        case_sensitive: true,
                        all_occurrences: true,
                    },
                ),
                step(Scope::Stem, Step::ChangeCase { mode: CaseMode::Upper }),
            ],
        };
        let (rows, _) = run_pipeline(&entries, &pipeline);
        assert_eq!(rows[0].new_name, "BAR.txt");
    }

    #[test]
    fn disabled_step_is_skipped() {
        let entries = vec![entry("a", "/dir", "foo", "txt")];
        let mut cfg = step(
            Scope::Stem,
            Step::FindReplace {
                find: "foo".to_string(),
                replace: "bar".to_string(),
                case_sensitive: true,
                all_occurrences: true,
            },
        );
        cfg.enabled = false;
        let pipeline = Pipeline { steps: vec![cfg] };
        let (rows, _) = run_pipeline(&entries, &pipeline);
        assert_eq!(rows[0].new_name, "foo.txt");
        assert_eq!(rows[0].status, RowStatus::Unchanged);
    }
}
