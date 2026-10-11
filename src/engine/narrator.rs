//! **The narrator** reads menus aloud: each screen's title as it opens, and
//! each button as the mouse (or a controller) comes to it, in the language
//! being shown. It uses the speech program the computer already has, so the
//! game still needs nothing more to build:
//!
//! - Windows: the built-in speech synthesiser, through PowerShell;
//! - macOS: `say`;
//! - Linux: `spd-say` (Speech Dispatcher), `espeak-ng` or `espeak`.
//!
//! The text goes to the program on its standard input (or, on Windows, in
//! an environment variable), never on a command line, so nothing in it can
//! be taken for an option. Something new to say cuts off what's being said.

use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};

/// What's speaking now (stopped when there's something new to say).
static SPEAKING: Mutex<Option<Child>> = Mutex::new(None);
/// The last few things said (for tests, and for the F3 screen).
static SAID: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[derive(Clone, Copy, Debug, PartialEq)]
enum Voice {
    PowerShell,
    Say,
    SpdSay,
    Espeak(&'static str),
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file() || d.join(format!("{program}.exe")).is_file()))
}

fn voice() -> Option<Voice> {
    static V: OnceLock<Option<Voice>> = OnceLock::new();
    *V.get_or_init(|| {
        if cfg!(windows) {
            Some(Voice::PowerShell)
        } else if cfg!(target_os = "macos") {
            Some(Voice::Say)
        } else if on_path("spd-say") {
            Some(Voice::SpdSay)
        } else if on_path("espeak-ng") {
            Some(Voice::Espeak("espeak-ng"))
        } else if on_path("espeak") {
            Some(Voice::Espeak("espeak"))
        } else {
            None
        }
    })
}

/// Can the narrator speak on this computer?
pub fn available() -> bool {
    voice().is_some()
}

/// Read `text` aloud, cutting off whatever is being read now.
pub fn say(text: &str) {
    let text: String = text.chars().filter(|c| !c.is_control()).take(400).collect();
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    if let Ok(mut said) = SAID.lock() {
        if said.len() >= 16 {
            said.remove(0);
        }
        said.push(text.to_string());
    }
    let Ok(mut speaking) = SPEAKING.lock() else { return };
    if let Some(mut old) = speaking.take() {
        let _ = old.kill();
        let _ = old.wait();
    }
    let Some(v) = voice() else { return };
    let mut cmd = match v {
        Voice::PowerShell => {
            let mut c = Command::new("powershell");
            c.args(["-NoProfile", "-NonInteractive", "-Command", "Add-Type -AssemblyName System.Speech; (New-Object System.Speech.Synthesis.SpeechSynthesizer).Speak($env:MINCERAFT_SAY)"]);
            c.env("MINCERAFT_SAY", text);
            c
        }
        Voice::Say => Command::new("say"),
        // (-e: read the text from standard input; -w: wait, so it can be cut off.)
        Voice::SpdSay => {
            let mut c = Command::new("spd-say");
            c.args(["-e", "-w"]);
            c
        }
        Voice::Espeak(p) => {
            let mut c = Command::new(p);
            c.arg("--stdin");
            c
        }
    };
    cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
    if let Ok(mut child) = cmd.spawn() {
        if let Some(mut input) = child.stdin.take() && v != Voice::PowerShell {
            let _ = input.write_all(text.as_bytes());
        }
        *speaking = Some(child);
    }
}

/// What was said most recently.
#[cfg(test)]
pub fn last_said() -> Option<String> {
    SAID.lock().ok().and_then(|s| s.last().cloned())
}

/// Keeps track of what's on screen and says what's new (see ui.rs: the
/// screen's title and the button under the pointer are noted as they're drawn).
#[derive(Default)]
pub struct Narration {
    title: String,
    focus: String,
}

impl Narration {
    /// Once a frame, with this frame's title and focused button (empty: none).
    /// Returns what to say: a new screen's title (and the button, if one is
    /// already under the pointer), or a newly focused button.
    pub fn update(&mut self, title: &str, focus: &str) -> Option<String> {
        let mut out = Vec::new();
        if title != self.title {
            self.title = title.to_string();
            if !title.is_empty() {
                out.push(title.to_string());
            }
            // (A button under the pointer on a new screen is said after the title.)
            self.focus.clear();
        }
        if focus != self.focus {
            self.focus = focus.to_string();
            if !focus.is_empty() {
                out.push(format!("{focus}, button"));
            }
        }
        (!out.is_empty()).then(|| out.join(". "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_screens_and_buttons_are_said_once() {
        let mut n = Narration::default();
        assert_eq!(n.update("Options", "").as_deref(), Some("Options"));
        assert_eq!(n.update("Options", ""), None);
        assert_eq!(n.update("Options", "Video Settings...").as_deref(), Some("Video Settings..., button"));
        assert_eq!(n.update("Options", "Video Settings..."), None);
        assert_eq!(n.update("Options", ""), None);
        assert_eq!(n.update("Video Settings", "Done").as_deref(), Some("Video Settings. Done, button"));
        assert_eq!(n.update("", ""), None);
    }

    #[test]
    fn what_is_said_is_kept_clean() {
        say("Hello\nthere\u{7}");
        assert_eq!(last_said().as_deref(), Some("Hellothere"));
        say("   ");
        assert_eq!(last_said().as_deref(), Some("Hellothere"));
    }
}
