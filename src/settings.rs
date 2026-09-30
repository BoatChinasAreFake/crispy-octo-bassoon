//! Options that outlive the game: `settings.txt`, one `key=value` per line.
//!
//! Hand-editable on purpose. Unknown keys are ignored, bad values fall back to
//! the defaults, and everything is clamped to what the Options screen allows,
//! so a mangled file can't do worse than reset a slider.

use std::path::{Path, PathBuf};

pub const FILE: &str = "settings.txt";
/// The farthest the world can be drawn, in chunks (greedy meshing keeps it affordable).
pub const MAX_RENDER_DISTANCE: i32 = 32;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub render_distance: i32,
    pub fov: f32,
    pub sensitivity: f32,
    pub fullscreen: bool,
    pub volume: f32,
    pub music_on: bool,
    /// Last name and server used on the Multiplayer screen (empty: pick a fresh name).
    pub mp_name: String,
    pub mp_addr: String,
    /// Keys and mouse buttons (see keybinds.rs).
    pub binds: crate::keybinds::Bindings,
    /// How you look (see nametags.rs).
    pub skin: u8,
    /// Captions for sounds, and colours that don't lean on red and green (see access.rs).
    pub subtitles: bool,
    pub colour_blind: bool,
    /// Graphics: leaves sway, water reflects the sky, light blends smoothly across faces.
    pub waving_leaves: bool,
    pub water_reflections: bool,
    pub smooth_lighting: bool,
    /// How much dim places are lifted: 0 moody, 1 bright (see light::shade).
    pub brightness: f32,
    /// Clouds as thick blocks (Fancy) or a flat layer (Fast).
    pub fancy_clouds: bool,
    /// Menus and HUD size, times the automatic size for the window (see ui.rs).
    pub ui_scale: f32,
    /// Ask GitHub whether there's a newer version (release builds only).
    pub check_updates: bool,
}

/// The UI Size choices (Options), as multipliers of the automatic size.
pub const UI_SCALES: [(f32, &str); 4] = [(0.8, "Small"), (1.0, "Normal"), (1.2, "Large"), (1.4, "Huge")];

impl Default for Settings {
    fn default() -> Self {
        Settings {
            render_distance: 8,
            fov: 72.0,
            sensitivity: 1.0,
            fullscreen: false,
            volume: 0.8,
            music_on: true,
            mp_name: String::new(),
            mp_addr: "127.0.0.1".into(),
            binds: Default::default(),
            skin: 0,
            subtitles: false,
            colour_blind: false,
            waving_leaves: true,
            water_reflections: true,
            smooth_lighting: true,
            brightness: crate::light::DEFAULT_BRIGHTNESS,
            fancy_clouds: true,
            ui_scale: 1.0,
            check_updates: true,
        }
    }
}

pub fn path() -> PathBuf {
    PathBuf::from(FILE)
}

/// One line of text, safe to write back out: no newlines or control characters.
fn clean(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

impl Settings {
    pub fn to_text(&self) -> String {
        format!(
            "# Minceraft settings. Edit freely; nonsense is quietly replaced with defaults.\n\
             render_distance={}\nfov={}\nsensitivity={}\nfullscreen={}\nvolume={}\nmusic={}\nname={}\nserver={}\nskin={}\nsubtitles={}\ncolour_blind={}\nwaving_leaves={}\nwater_reflections={}\nsmooth_lighting={}\nbrightness={}\nfancy_clouds={}\nui_scale={}\ncheck_updates={}\n",
            self.render_distance,
            self.fov,
            self.sensitivity,
            self.fullscreen,
            self.volume,
            self.music_on,
            clean(&self.mp_name, 16),
            clean(&self.mp_addr, 128),
            self.skin,
            self.subtitles,
            self.colour_blind,
            self.waving_leaves,
            self.water_reflections,
            self.smooth_lighting,
            self.brightness,
            self.fancy_clouds,
            self.ui_scale,
            self.check_updates,
        ) + self.binds.to_text().as_str()
    }

    pub fn from_text(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            let num = |d: f32| v.parse::<f32>().ok().filter(|x| x.is_finite()).unwrap_or(d);
            let flag = |d: bool| match v {
                "true" | "on" | "yes" | "1" => true,
                "false" | "off" | "no" | "0" => false,
                _ => d,
            };
            match k.trim() {
                "render_distance" => s.render_distance = v.parse().unwrap_or(s.render_distance),
                "fov" => s.fov = num(s.fov),
                "sensitivity" => s.sensitivity = num(s.sensitivity),
                "fullscreen" => s.fullscreen = flag(s.fullscreen),
                "volume" => s.volume = num(s.volume),
                "music" => s.music_on = flag(s.music_on),
                "name" => s.mp_name = clean(v, 16),
                "server" => s.mp_addr = clean(v, 128),
                "skin" => s.skin = v.parse::<u8>().unwrap_or(0) % crate::nametags::SKINS.len() as u8,
                "subtitles" => s.subtitles = flag(s.subtitles),
                "colour_blind" | "color_blind" => s.colour_blind = flag(s.colour_blind),
                "waving_leaves" => s.waving_leaves = flag(s.waving_leaves),
                "water_reflections" => s.water_reflections = flag(s.water_reflections),
                "smooth_lighting" => s.smooth_lighting = flag(s.smooth_lighting),
                "brightness" => s.brightness = num(s.brightness),
                "fancy_clouds" => s.fancy_clouds = flag(s.fancy_clouds),
                "ui_scale" => s.ui_scale = num(s.ui_scale),
                "check_updates" => s.check_updates = flag(s.check_updates),
                k => {
                    s.binds.read(k, v);
                }
            }
        }
        s.render_distance = s.render_distance.clamp(3, MAX_RENDER_DISTANCE);
        s.fov = s.fov.clamp(50.0, 110.0);
        s.sensitivity = s.sensitivity.clamp(0.1, 3.0);
        s.volume = s.volume.clamp(0.0, 1.0);
        s.brightness = s.brightness.clamp(0.0, 1.0);
        s.ui_scale = s.ui_scale.clamp(UI_SCALES[0].0, UI_SCALES[UI_SCALES.len() - 1].0);
        if s.mp_addr.is_empty() {
            s.mp_addr = Settings::default().mp_addr;
        }
        s
    }

    /// Missing or unreadable: the defaults.
    pub fn load(path: &Path) -> Settings {
        std::fs::read_to_string(path).map(|t| Settings::from_text(&t)).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.to_text())?;
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let s = Settings {
            render_distance: 12,
            fov: 95.0,
            sensitivity: 1.7,
            fullscreen: true,
            volume: 0.3,
            music_on: false,
            mp_name: "Stove42".into(),
            mp_addr: "[::1]:25565".into(),
            binds: {
                let mut b = crate::keybinds::Bindings::default();
                b.set(crate::keybinds::Action::Sprint, false, crate::keybinds::Bind::parse("F"));
                b
            },
            skin: 3,
            subtitles: true,
            colour_blind: true,
            waving_leaves: false,
            water_reflections: false,
            smooth_lighting: false,
            brightness: 0.8,
            fancy_clouds: false,
            ui_scale: 1.2,
            check_updates: false,
        };
        assert_eq!(Settings::from_text(&s.to_text()), s);
        let dir = std::env::temp_dir().join(format!("minceraft-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(FILE);
        s.save(&p).unwrap();
        assert_eq!(Settings::load(&p), s);
        assert_eq!(Settings::load(&dir.join("nope.txt")), Settings::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nonsense_is_tamed() {
        let s = Settings::from_text("render_distance=9000\nfov=NaN\nvolume=-3\nmusic=perhaps\nname=a\u{7}b\nserver=\n= \ngarbage\nsensitivity=2");
        assert_eq!(s.render_distance, MAX_RENDER_DISTANCE);
        assert_eq!(s.fov, 72.0);
        assert_eq!(s.volume, 0.0);
        assert!(s.music_on);
        assert_eq!(s.mp_name, "ab");
        assert_eq!(s.mp_addr, "127.0.0.1");
        assert_eq!(s.sensitivity, 2.0);
        // A name can't smuggle in extra lines.
        let evil = Settings { mp_name: "x\nfullscreen=true".into(), ..Settings::default() };
        assert!(!Settings::from_text(&evil.to_text()).fullscreen);
    }
}
