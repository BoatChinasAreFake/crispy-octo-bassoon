//! Where the game keeps its files, and the crash log.
//!
//! Worlds, settings and mods live in the player's own data folder
//! (`%APPDATA%\Minceraft` on Windows, `~/Library/Application Support/Minceraft`
//! on macOS, `~/.local/share/minceraft` on Linux), so a new version can be
//! unzipped anywhere and still finds them. The game simply works from inside
//! that folder: every path elsewhere (`saves/`, `settings.txt`, `mods/`) stays
//! relative. A `portable.txt` next to the game keeps everything beside it
//! instead, as older versions did. The dedicated server always uses the folder
//! it's started in.

use std::path::{Path, PathBuf};

/// What gets carried over from beside the game on first run.
const CARRIED: [&str; 3] = ["saves", "settings.txt", "mods"];
/// Put this file next to the game to keep its files there.
pub const PORTABLE_FILE: &str = "portable.txt";
/// The crash log, in the data folder.
pub const CRASH_FILE: &str = "crash.txt";

/// The game's version: the release tag for release builds.
pub fn version() -> &'static str {
    option_env!("MINCERAFT_VERSION").unwrap_or(concat!("dev ", env!("CARGO_PKG_VERSION")))
}

/// The per-player data folder for this system, if the environment says where.
pub fn user_data_dir() -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        var("APPDATA").map(|d| d.join("Minceraft"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Application Support/Minceraft"))
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|h| h.join(".local/share"))).map(|d| d.join("minceraft"))
    }
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(Path::to_path_buf)
}

/// Move into the data folder (making it, and carrying over anything saved
/// beside the game the first time). Returns the folder the game now works in,
/// and notes on anything that went wrong (the game carries on regardless).
pub fn enter_data_dir() -> (PathBuf, Vec<String>) {
    let here = std::env::current_dir().unwrap_or_default();
    let beside: Vec<PathBuf> = {
        let mut v = vec![here.clone()];
        if let Some(e) = exe_dir()
            && !v.contains(&e)
        {
            v.push(e);
        }
        v
    };
    if beside.iter().any(|d| d.join(PORTABLE_FILE).is_file()) {
        return (here, Vec::new());
    }
    let Some(dir) = user_data_dir() else { return (here, vec!["No user data folder on this system; keeping files beside the game.".into()]) };
    let mut notes = Vec::new();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return (here, vec![format!("Couldn't make {}: {e}; keeping files beside the game.", dir.display())]);
    }
    notes.extend(carry_over(&beside, &dir));
    match std::env::set_current_dir(&dir) {
        Ok(()) => (dir, notes),
        Err(e) => {
            notes.push(format!("Couldn't use {}: {e}; keeping files beside the game.", dir.display()));
            (here, notes)
        }
    }
}

/// Copy saves, settings and mods from the first of `from` that has each into
/// `to`, unless `to` already has it. Originals are left alone.
pub fn carry_over(from: &[PathBuf], to: &Path) -> Vec<String> {
    let mut notes = Vec::new();
    for name in CARRIED {
        let dest = to.join(name);
        if dest.exists() {
            continue;
        }
        let Some(src) = from.iter().map(|d| d.join(name)).find(|p| p.exists() && p != &dest) else { continue };
        if let Err(e) = copy_all(&src, &dest) {
            notes.push(format!("Couldn't copy {} to {}: {e}", src.display(), dest.display()));
        }
    }
    notes
}

pub fn copy_all(src: &Path, dest: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dest)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            copy_all(&entry.path(), &dest.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(src, dest).map(|_| ())
    }
}

/// Write a crash report for a panic: what happened, where, and the stack.
pub fn write_crash_report(info: &std::panic::PanicHookInfo) -> Option<PathBuf> {
    let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
    let text = crash_report(&info.to_string(), &thread, &std::backtrace::Backtrace::force_capture().to_string());
    let path = std::env::current_dir().unwrap_or_default().join(CRASH_FILE);
    std::fs::write(&path, text).ok()?;
    Some(path)
}

fn crash_report(what: &str, thread: &str, stack: &str) -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!(
        "Minceraft crashed. Please include this whole file when reporting it.\n\n\
         Version: {}\nSystem: {} {}\nTime: {} UTC\nThread: {thread}\n\n\
         What happened:\n{what}\n\nStack:\n{stack}\n",
        version(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        utc_date_time(secs),
    )
}

/// "YYYY-MM-DD hh:mm:ss" for seconds since 1970 (UTC).
pub fn utc_date_time(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil date from a day count (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", rem / 3600, rem / 60 % 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_come_out_right() {
        assert_eq!(utc_date_time(0), "1970-01-01 00:00:00");
        assert_eq!(utc_date_time(951_782_400), "2000-02-29 00:00:00");
        assert_eq!(utc_date_time(1_790_000_000), "2026-09-21 14:13:20");
    }

    #[test]
    fn crash_reports_say_what_where_and_which_version() {
        let r = crash_report("panicked at src/game.rs:10:5:\nboom", "main", "0: minceraft::game::tick");
        assert!(r.contains(version()) && r.contains("boom") && r.contains("src/game.rs:10") && r.contains("game::tick"));
    }

    #[test]
    fn first_run_carries_saves_over_once() {
        let root = std::env::temp_dir().join(format!("minceraft-paths-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (old, data) = (root.join("old"), root.join("data"));
        std::fs::create_dir_all(old.join("saves/my_world")).unwrap();
        std::fs::write(old.join("saves/my_world/world.mncr"), "blocks").unwrap();
        std::fs::write(old.join("settings.txt"), "fov=90\n").unwrap();
        std::fs::create_dir_all(&data).unwrap();
        assert!(carry_over(std::slice::from_ref(&old), &data).is_empty());
        assert_eq!(std::fs::read_to_string(data.join("saves/my_world/world.mncr")).unwrap(), "blocks");
        assert_eq!(std::fs::read_to_string(data.join("settings.txt")).unwrap(), "fov=90\n");
        assert!(old.join("saves/my_world/world.mncr").exists(), "originals stay");
        // A world deleted in the data folder later doesn't come back from beside the game.
        std::fs::remove_dir_all(data.join("saves/my_world")).unwrap();
        carry_over(std::slice::from_ref(&old), &data);
        assert!(!data.join("saves/my_world").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
