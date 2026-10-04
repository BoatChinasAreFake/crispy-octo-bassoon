//! "A new version is out": release builds ask GitHub for the latest release
//! once, in the background, and the title screen offers a download button when
//! it's newer. The system's own `curl` does the asking (Windows 10+, macOS and
//! Linux all have one), so the game needs no web library; if it's missing or
//! offline, nothing happens. Turn it off in Options (Update Check).

use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

const REPO: &str = "BoatChinasAreFake/crispy-octo-bassoon";

/// Where to download the newest version.
pub fn releases_page() -> String {
    format!("https://github.com/{REPO}/releases/latest")
}

pub struct UpdateCheck {
    rx: Option<Receiver<Option<String>>>,
    /// The newer version's tag, once known ("v0.1.4").
    pub newer: Option<String>,
}

impl UpdateCheck {
    /// Start asking (release builds only; dev builds have nothing to compare).
    pub fn start(enabled: bool) -> UpdateCheck {
        let current = crate::paths::version();
        if !enabled || parse(current).is_none() {
            return UpdateCheck { rx: None, newer: None };
        }
        let (tx, rx) = channel();
        let _ = std::thread::Builder::new().name("update-check".into()).spawn(move || {
            let _ = tx.send(latest_tag().filter(|latest| is_newer(latest, current)));
        });
        UpdateCheck { rx: Some(rx), newer: None }
    }

    /// Pick up the answer when it arrives (call once a frame).
    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx
            && let Ok(answer) = rx.try_recv()
        {
            self.newer = answer;
            self.rx = None;
        }
    }
}

fn quiet(cmd: &mut Command) -> &mut Command {
    cmd.stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        // No console window flashing up for the helper.
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// The latest release's tag, from GitHub's API (None if anything goes wrong).
fn latest_tag() -> Option<String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let out = quiet(Command::new("curl").args(["-fsSL", "-m", "10", "-H", "Accept: application/vnd.github+json", "-A", "minceraft-update-check", &url])).output().ok()?;
    if !out.status.success() {
        return None;
    }
    tag_in(&String::from_utf8_lossy(&out.stdout))
}

/// `"tag_name": "v0.1.4"` out of the API's JSON.
fn tag_in(json: &str) -> Option<String> {
    let rest = &json[json.find("\"tag_name\"")? + 10..];
    let rest = &rest[rest.find('"')? + 1..];
    let tag = &rest[..rest.find('"')?];
    parse(tag).map(|_| tag.to_string())
}

/// "v1.2.3" as numbers.
fn parse(v: &str) -> Option<(u32, u32, u32)> {
    let mut n = v.strip_prefix('v')?.split('.').map(|p| p.parse::<u32>().ok());
    Some((n.next()??, n.next().flatten().unwrap_or(0), n.next().flatten().unwrap_or(0)))
}

fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse(latest), parse(current)), (Some(l), Some(c)) if l > c)
}

/// Open a web page in the player's browser.
pub fn open_in_browser(url: &str) {
    let result = if cfg!(windows) {
        quiet(Command::new("cmd").args(["/C", "start", "", url])).spawn()
    } else if cfg!(target_os = "macos") {
        quiet(Command::new("open").arg(url)).spawn()
    } else {
        quiet(Command::new("xdg-open").arg(url)).spawn()
    };
    if let Err(e) = result {
        eprintln!("Minceraft: couldn't open {url}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("v0.1.10", "v0.1.9"));
        assert!(is_newer("v0.2.0", "v0.1.3"));
        assert!(is_newer("v1.0", "v0.9.9"));
        assert!(!is_newer("v0.1.3", "v0.1.3"));
        assert!(!is_newer("v0.1.2", "v0.1.3"));
        assert!(!is_newer("v0.1.4", "dev 0.1.0"), "dev builds never nag");
        assert!(!is_newer("nightly", "v0.1.3"));
    }

    #[test]
    fn the_tag_comes_out_of_the_json() {
        let json = r#"{"url":"x","tag_name": "v0.1.4","name":"Minceraft v0.1.4","draft":false}"#;
        assert_eq!(tag_in(json).as_deref(), Some("v0.1.4"));
        assert_eq!(tag_in(r#"{"message":"Not Found"}"#), None);
        assert_eq!(tag_in(r#"{"tag_name":"latest"}"#), None);
    }

    #[test]
    fn dev_builds_and_disabled_checks_stay_offline() {
        let mut c = UpdateCheck::start(false);
        c.poll();
        assert!(c.rx.is_none() && c.newer.is_none());
    }
}
