use crate::errors::AppError;
use crate::session::{FlatBlock, flatten_workout};
use crate::workout::{ParsedWorkout, parse_zwo};
use serde::Serialize;
use specta::Type;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{read_dir, read_to_string};

/// FTP used to flatten FTP-test workouts, so watts equal percent on their cards.
/// Must match FTP_TEST_REFERENCE_W in src/lib/ftp.ts.
const FTP_TEST_REFERENCE_W: u16 = 100;

/// A file that could not be read or parsed, returned alongside successful workouts.
#[derive(Debug, Serialize, Type)]
pub struct WorkoutFileError {
    pub file_name: String,
    pub message: String,
}

/// Result of listing a workout folder: successfully parsed workouts plus any
/// files that failed (parse errors, unreadable files), so the frontend can
/// show a warning without hiding the workouts that did load.
#[derive(Debug, Serialize, Type)]
pub struct WorkoutLibrary {
    pub workouts: Vec<ParsedWorkout>,
    /// Flat blocks parallel to `workouts`, flattened at each card's FTP (reference
    /// FTP for tests) so thumbnails render exactly what the session will run.
    pub flats: Vec<Vec<FlatBlock>>,
    pub errors: Vec<WorkoutFileError>,
}

/// Diffs a disk listing against the DB cache: decides what needs reparsing
/// (new or changed mtime), what's stale (gone from disk), what's reusable.
pub(crate) struct ReconcilePlan {
    pub(crate) to_parse: Vec<String>,
    pub(crate) to_delete: Vec<String>,
    pub(crate) unchanged: Vec<String>,
}

pub(crate) fn reconcile(
    disk: &[(String, i64)],
    cached: &HashMap<String, (i64, String)>,
) -> ReconcilePlan {
    let mut to_parse = Vec::new();
    let mut unchanged = Vec::new();
    for (file_name, mtime) in disk {
        match cached.get(file_name) {
            Some((cached_mtime, _)) if cached_mtime == mtime => unchanged.push(file_name.clone()),
            _ => to_parse.push(file_name.clone()),
        }
    }
    let disk_names: std::collections::HashSet<_> = disk.iter().map(|(n, _)| n).collect();
    let to_delete = cached
        .keys()
        .filter(|k| !disk_names.contains(k))
        .cloned()
        .collect();
    ReconcilePlan {
        to_parse,
        to_delete,
        unchanged,
    }
}

/// Result of a cache-aware library scan: the library itself, plus what changed
/// so the caller (which owns the DB handle) can persist it.
pub(crate) struct ReconciledLibrary {
    pub(crate) result: WorkoutLibrary,
    /// (file_name, mtime_secs, parsed_json) for files that were (re)parsed.
    pub(crate) to_upsert: Vec<(String, i64, String)>,
    pub(crate) to_delete: Vec<String>,
}

/// Lists a workout folder, reparsing only files that are new or whose mtime
/// changed since `cached` was last persisted; reuses parsed JSON otherwise.
pub(crate) fn list_workouts_cached(
    folder: &str,
    ftp_w: u16,
    cached: HashMap<String, (i64, String)>,
) -> Result<ReconciledLibrary, AppError> {
    let DiskScan {
        entries: disk,
        mtimes,
    } = scan_disk(folder)?;
    let plan = reconcile(&disk, &cached);

    let mut workouts = Vec::new();
    let mut flats = Vec::new();
    let mut errors = Vec::new();
    let mut to_upsert = Vec::new();

    load_unchanged(
        &plan.unchanged,
        &cached,
        ftp_w,
        &mut workouts,
        &mut flats,
        &mut errors,
    );
    parse_changed(
        &plan.to_parse,
        folder,
        &mtimes,
        ftp_w,
        &mut workouts,
        &mut flats,
        &mut errors,
        &mut to_upsert,
    );

    Ok(ReconciledLibrary {
        result: WorkoutLibrary {
            workouts,
            flats,
            errors,
        },
        to_upsert,
        to_delete: plan.to_delete,
    })
}

/// Disk listing plus a name-indexed mtime lookup, built together in one pass.
struct DiskScan {
    entries: Vec<(String, i64)>,
    mtimes: HashMap<String, i64>,
}

/// Lists `.zwo` file names and their mtime (seconds since epoch) in `folder`.
fn scan_disk(folder: &str) -> Result<DiskScan, AppError> {
    let entries = read_dir(folder)?;
    let mut disk = Vec::new();
    let mut mtimes = HashMap::new();
    for entry in entries.flatten() {
        if entry.path().extension() != Some(OsStr::new("zwo")) {
            continue;
        }
        let file_name = entry
            .path()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // -1 never matches a real mtime (always >= 0), so an unreadable mtime
        // always reparses instead of risking a false "unchanged" on 0.
        let mtime = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(-1);
        mtimes.insert(file_name.clone(), mtime);
        disk.push((file_name, mtime));
    }
    Ok(DiskScan {
        entries: disk,
        mtimes,
    })
}

fn load_unchanged(
    unchanged: &[String],
    cached: &HashMap<String, (i64, String)>,
    ftp_w: u16,
    workouts: &mut Vec<ParsedWorkout>,
    flats: &mut Vec<Vec<FlatBlock>>,
    errors: &mut Vec<WorkoutFileError>,
) {
    for file_name in unchanged {
        let Some((_, json)) = cached.get(file_name) else {
            continue;
        };
        match serde_json::from_str::<ParsedWorkout>(json) {
            Ok(w) => push_workout(w, ftp_w, workouts, flats),
            Err(e) => errors.push(WorkoutFileError {
                file_name: file_name.clone(),
                message: e.to_string(),
            }),
        }
    }
}

#[allow(clippy::too_many_arguments)] // internal helper, all params are single-use accumulators
fn parse_changed(
    to_parse: &[String],
    folder: &str,
    mtimes: &HashMap<String, i64>,
    ftp_w: u16,
    workouts: &mut Vec<ParsedWorkout>,
    flats: &mut Vec<Vec<FlatBlock>>,
    errors: &mut Vec<WorkoutFileError>,
    to_upsert: &mut Vec<(String, i64, String)>,
) {
    for file_name in to_parse {
        let path = std::path::Path::new(folder).join(file_name);
        let parsed = read_to_string(&path)
            .map_err(AppError::from)
            .and_then(|c| parse_zwo(&c));
        match parsed {
            Ok(mut w) => {
                w.file_name = Some(file_name.clone());
                if let (Ok(json), Some(&mtime)) = (serde_json::to_string(&w), mtimes.get(file_name))
                {
                    to_upsert.push((file_name.clone(), mtime, json));
                }
                push_workout(w, ftp_w, workouts, flats);
            }
            Err(e) => errors.push(WorkoutFileError {
                file_name: file_name.clone(),
                message: e.to_string(),
            }),
        }
    }
}

fn push_workout(
    w: ParsedWorkout,
    ftp_w: u16,
    workouts: &mut Vec<ParsedWorkout>,
    flats: &mut Vec<Vec<FlatBlock>>,
) {
    let card_ftp = if w.is_ftp_test {
        FTP_TEST_REFERENCE_W
    } else {
        ftp_w
    };
    flats.push(flatten_workout(w.clone(), card_ftp));
    workouts.push(w);
}

#[cfg(test)]
mod reconcile_tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn reconcile_classifies_new_changed_unchanged_and_deleted() {
        let disk = vec![
            ("new.zwo".to_string(), 100),
            ("changed.zwo".to_string(), 200),
            ("same.zwo".to_string(), 300),
        ];
        let mut cached = HashMap::new();
        cached.insert("changed.zwo".to_string(), (150, "{}".to_string()));
        cached.insert("same.zwo".to_string(), (300, "{}".to_string()));
        cached.insert("gone.zwo".to_string(), (50, "{}".to_string()));

        let plan = reconcile(&disk, &cached);

        assert_eq!(
            plan.to_parse,
            vec!["new.zwo".to_string(), "changed.zwo".to_string()]
        );
        assert_eq!(plan.unchanged, vec!["same.zwo".to_string()]);
        assert_eq!(plan.to_delete, vec!["gone.zwo".to_string()]);
    }
}

#[cfg(test)]
mod list_workouts_cached_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const ZWO: &str = r#"<workout_file>
        <sportType>bike</sportType>
        <workout><SteadyState Duration="60" Power="0.5"/></workout>
    </workout_file>"#;

    fn unique_dir(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("opencycling_test_{name}_{nonce}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Pins the caching contract: a second scan of an untouched folder reparses
    /// nothing (0 `to_upsert`), which is what the manual UI verification checks.
    #[test]
    fn second_scan_of_untouched_folder_reparses_nothing() {
        let dir = unique_dir("second_scan");
        std::fs::write(dir.join("a.zwo"), ZWO).unwrap();

        let first = list_workouts_cached(dir.to_str().unwrap(), 200, HashMap::new()).unwrap();
        assert_eq!(first.to_upsert.len(), 1, "first scan must parse the file");
        assert!(first.result.errors.is_empty());

        let cached: HashMap<_, _> = first
            .to_upsert
            .into_iter()
            .map(|(name, mtime, json)| (name, (mtime, json)))
            .collect();
        let second = list_workouts_cached(dir.to_str().unwrap(), 200, cached).unwrap();

        assert!(
            second.to_upsert.is_empty(),
            "unchanged file must not be reparsed"
        );
        assert!(second.to_delete.is_empty());
        assert_eq!(second.result.workouts.len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }
}
