//! World backups: each time a world is opened, a copy of its folder as it was
//! goes into `backups/<world>/<date>/` (in the data folder, beside `saves/`).
//! The newest few are kept per world. Restoring makes a new world from a copy,
//! so nothing is ever overwritten.

use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// How many backups each world keeps.
pub const KEEP: usize = 5;

pub fn backups_dir() -> PathBuf {
    PathBuf::from("backups")
}

pub struct Backup {
    pub path: PathBuf,
    /// "2026-09-30 14:05:09" (UTC), from the folder name.
    pub when: String,
    pub made: Option<SystemTime>,
    pub size: u64,
}

/// Folder names sort by date: "2026-09-30_14-05-09".
fn stamp(secs: u64) -> String {
    crate::paths::utc_date_time(secs).replace(' ', "_").replace(':', "-")
}

fn unstamp(name: &str) -> Option<String> {
    let (d, t) = name.split_once('_')?;
    (d.len() == 10 && t.len() == 8 && name.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '_')).then(|| format!("{d} {}", t.replace('-', ":")))
}

/// Copy the world `id` from `saves` into a new backup (made at `now`, seconds
/// since 1970), then drop the oldest beyond `KEEP`. Nothing to back up (a
/// world that hasn't been saved yet) is fine: returns None.
pub fn back_up(saves: &Path, backups: &Path, id: &str, now: u64) -> io::Result<Option<PathBuf>> {
    if !crate::save::valid_id(id) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "bad world id"));
    }
    let src = saves.join(id);
    if !crate::save::world_file(saves, id).is_file() {
        return Ok(None);
    }
    let dest = backups.join(id).join(stamp(now));
    if dest.exists() {
        return Ok(Some(dest));
    }
    // Copy into a scratch folder first, so a half-made backup never looks real.
    let part = backups.join(id).join(format!("{}.partial", stamp(now)));
    let _ = std::fs::remove_dir_all(&part);
    crate::paths::copy_all(&src, &part)?;
    std::fs::rename(&part, &dest)?;
    for old in list(backups, id).into_iter().skip(KEEP) {
        let _ = std::fs::remove_dir_all(old.path);
    }
    Ok(Some(dest))
}

/// A world's backups, newest first.
pub fn list(backups: &Path, id: &str) -> Vec<Backup> {
    let Ok(entries) = std::fs::read_dir(backups.join(id)) else { return Vec::new() };
    let mut out: Vec<Backup> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let when = unstamp(&name)?;
            let path = e.path();
            path.is_dir().then(|| Backup { made: made_at(&when), size: tree_size(&path), when, path })
        })
        .collect();
    out.sort_by(|a, b| b.when.cmp(&a.when));
    out
}

/// "2026-09-30 14:05:09" (UTC) back to a time.
fn made_at(when: &str) -> Option<SystemTime> {
    let n: Vec<i64> = when.split(|c: char| !c.is_ascii_digit()).map(|p| p.parse().ok()).collect::<Option<_>>()?;
    let [y, m, d, hh, mm, ss] = n[..] else { return None };
    // Days since 1970 for a civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = u64::try_from(days * 86_400 + hh * 3600 + mm * 60 + ss).ok()?;
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
}

fn tree_size(p: &Path) -> u64 {
    match std::fs::read_dir(p) {
        Ok(es) => es.flatten().map(|e| tree_size(&e.path())).sum(),
        Err(_) => std::fs::metadata(p).map(|m| m.len()).unwrap_or(0),
    }
}

/// Make a new world from a backup, named "<name> (backup <date>)". Returns its id.
pub fn restore_as_copy(saves: &Path, backup: &Backup, name: &str) -> io::Result<String> {
    let new_name = crate::save::clean_name(&format!("{name} (backup {})", &backup.when[..16.min(backup.when.len())]));
    let id = crate::save::new_world_id(saves, &new_name);
    crate::paths::copy_all(&backup.path, &saves.join(&id))?;
    crate::save::write_name(saves, &id, &new_name)?;
    Ok(id)
}

/// A deleted world's backups go with it.
pub fn delete_all(backups: &Path, id: &str) -> io::Result<()> {
    if !crate::save::valid_id(id) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "bad world id"));
    }
    match std::fs::remove_dir_all(backups.join(id)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backups_keep_the_newest_and_restore_as_new_worlds() {
        let root = std::env::temp_dir().join(format!("minceraft-backups-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (saves, backups) = (root.join("saves"), root.join("backups"));
        crate::save::write_name(&saves, "home", "Home").unwrap();
        // Not saved yet: nothing to back up.
        assert!(back_up(&saves, &backups, "home", 1_000).unwrap().is_none());
        std::fs::write(crate::save::world_file(&saves, "home"), "v1").unwrap();
        std::fs::create_dir_all(saves.join("home/world.regions")).unwrap();
        std::fs::write(saves.join("home/world.regions/r.0.0"), "edits").unwrap();
        for day in 0..(KEEP as u64 + 2) {
            std::fs::write(crate::save::world_file(&saves, "home"), format!("v{day}")).unwrap();
            back_up(&saves, &backups, "home", 1_790_000_000 + day * 86_400).unwrap();
        }
        let list = list(&backups, "home");
        assert_eq!(list.len(), KEEP, "only the newest few stay");
        assert_eq!(list[0].when, "2026-09-27 14:13:20");
        let made = list[0].made.unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        assert_eq!(made, 1_790_000_000 + (KEEP as u64 + 1) * 86_400, "the time comes back from the folder name");
        assert_eq!(std::fs::read_to_string(list[0].path.join("world.mncr")).unwrap(), format!("v{}", KEEP + 1));
        assert!(list[0].path.join("world.regions/r.0.0").is_file(), "region files come along");
        assert!(list[0].size > 0);

        let id = restore_as_copy(&saves, &list[KEEP - 1], "Home").unwrap();
        assert_ne!(id, "home", "restoring never overwrites");
        assert_eq!(crate::save::read_name(&saves, &id), "Home (backup 2026-09-23 14:13)");
        assert_eq!(std::fs::read_to_string(crate::save::world_file(&saves, &id)).unwrap(), "v2");
        assert_eq!(std::fs::read_to_string(crate::save::world_file(&saves, "home")).unwrap(), format!("v{}", KEEP + 1));

        delete_all(&backups, "home").unwrap();
        assert!(super::list(&backups, "home").is_empty());
        delete_all(&backups, "home").unwrap();
        std::fs::remove_dir_all(&root).unwrap();
    }
}
