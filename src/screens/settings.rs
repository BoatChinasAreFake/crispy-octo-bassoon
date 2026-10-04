//! Options: video, controls and the world's rules.

use crate::*;

impl App {
    pub(crate) fn options_screen(&mut self, from_title: bool) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        let bw = (220.0 * s).min(w * 0.8);
        // Rows shrink a little on short windows so Done stays on screen.
        let bh = (20.0 * s).min((h - 100.0 * s) / 10.0);
        let x = w / 2.0 - bw / 2.0;
        let total = 8.0 * bh + 7.0 * 5.0 * s + 7.0 * s;
        let mut y = ((h - total) / 2.0 + 10.0 * s).max(30.0 * s);
        self.ui.text_centered("Options", w / 2.0, y - 18.0 * s, 16.0, WHITE);
        let d = self.stepper(&format!("FOV: {:.0}", self.settings.fov), x, y, bw, bh);
        self.settings.fov = (self.settings.fov + 5.0 * d as f32).clamp(50.0, 110.0);
        y += bh + 5.0 * s;
        let d = self.stepper(&format!("Mouse Sensitivity: {:.0}%", self.settings.sensitivity * 100.0), x, y, bw, bh);
        self.settings.sensitivity = (self.settings.sensitivity + 0.1 * d as f32).clamp(0.1, 3.0);
        y += bh + 5.0 * s;
        let d = self.stepper(&format!("Sound Volume: {:.0}%", self.audio.volume * 100.0), x, y, bw, bh);
        let vol = &mut self.audio.volume;
        *vol = ((*vol + 0.1 * d as f32).clamp(0.0, 1.0) * 10.0).round() / 10.0;
        y += bh + 5.0 * s;
        // Two to a row from here on.
        let half = (bw - 4.0 * s) / 2.0;
        let (left, right) = (x, x + half + 4.0 * s);
        let music = if self.audio.music_on { "Music: ON" } else { "Music: OFF" };
        if self.ui.button(Rect::new(left, y, half, bh), music, true) {
            self.audio.music_on = !self.audio.music_on;
        }
        let skin = format!("Skin: {}", nametags::skin_name(self.settings.skin));
        if self.ui.button(Rect::new(right, y, half, bh), &skin, true) {
            self.settings.skin = (self.settings.skin + 1) % nametags::SKINS.len() as u8;
            let k = self.settings.skin;
            self.game.set_skin(k);
        }
        y += bh + 5.0 * s;
        if self.ui.button(Rect::new(left, y, half, bh), "Video Settings...", true) {
            self.set_screen(Screen::Video { from_title });
        }
        if self.ui.button(Rect::new(right, y, half, bh), "Controls...", true) {
            self.set_screen(Screen::Controls { from_title });
        }
        y += bh + 5.0 * s;
        let subs = if self.settings.subtitles { "Subtitles: ON" } else { "Subtitles: OFF" };
        if self.ui.button(Rect::new(left, y, half, bh), subs, true) {
            self.settings.subtitles = !self.settings.subtitles;
        }
        let cb = if self.settings.colour_blind { "Colour-blind: ON" } else { "Colour-blind: OFF" };
        if self.ui.button(Rect::new(right, y, half, bh), cb, true) {
            self.settings.colour_blind = !self.settings.colour_blind;
        }
        y += bh + 5.0 * s;
        let size_i = settings::UI_SCALES.iter().position(|(m, _)| (*m - self.settings.ui_scale).abs() < 0.01).unwrap_or(1);
        if self.ui.button(Rect::new(left, y, half, bh), &format!("UI Size: {}", settings::UI_SCALES[size_i].1), true) {
            self.settings.ui_scale = settings::UI_SCALES[(size_i + 1) % settings::UI_SCALES.len()].0;
        }
        let updates = if self.settings.check_updates { "Update Check: ON" } else { "Update Check: OFF" };
        if self.ui.button(Rect::new(right, y, half, bh), updates, true) {
            self.settings.check_updates = !self.settings.check_updates;
        }
        y += bh + 12.0 * s;
        if self.ui.button(Rect::new(x, y, bw, bh), "Done", true) {
            self.set_screen(if from_title { Screen::Title } else { Screen::Paused });
        }
    }

    /// Everything about how the game looks and how fast it draws.
    pub(crate) fn video_screen(&mut self, from_title: bool) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        let bw = (240.0 * s).min(w * 0.85);
        let bh = (20.0 * s).min((h - 100.0 * s) / 12.0);
        let x = w / 2.0 - bw / 2.0;
        let total = 10.0 * bh + 9.0 * 5.0 * s + 20.0 * s;
        let mut y = ((h - total) / 2.0 + 10.0 * s).max(30.0 * s);
        self.ui.text_centered("Video Settings", w / 2.0, y - 18.0 * s, 16.0, WHITE);
        let st = self.settings.clone();
        let d = self.stepper(&format!("Render Distance: {} chunks", st.render_distance), x, y, bw, bh);
        self.settings.render_distance = (st.render_distance + d).clamp(3, settings::MAX_RENDER_DISTANCE);
        y += bh + 5.0 * s;
        let bright = match (st.brightness * 100.0).round() as i32 {
            0 => "Moody".to_string(),
            100 => "Bright".to_string(),
            p => format!("{p}%"),
        };
        let d = self.stepper(&format!("Brightness: {bright}"), x, y, bw, bh);
        self.settings.brightness = (((st.brightness + 0.1 * d as f32).clamp(0.0, 1.0)) * 10.0).round() / 10.0;
        y += bh + 5.0 * s;
        let caps = settings::FPS_CAPS;
        let ci = caps.iter().position(|&c| c == st.max_fps).unwrap_or(caps.len() - 1);
        let label = if st.max_fps == 0 { "Max FPS: Unlimited".to_string() } else { format!("Max FPS: {}", st.max_fps) };
        let d = self.stepper(&label, x, y, bw, bh);
        self.settings.max_fps = caps[(ci as i32 + d).clamp(0, caps.len() as i32 - 1) as usize];
        y += bh + 5.0 * s;
        let half = (bw - 4.0 * s) / 2.0;
        let (left, right) = (x, x + half + 4.0 * s);
        let on_off = |name: &str, on: bool| format!("{name}: {}", if on { "ON" } else { "OFF" });
        if self.ui.button(Rect::new(left, y, half, bh), &on_off("Fullscreen", st.fullscreen), true) {
            self.settings.fullscreen = !st.fullscreen;
            set_fullscreen(self.settings.fullscreen);
        }
        if self.ui.button(Rect::new(right, y, half, bh), &on_off("VSync", st.vsync), true) {
            self.settings.vsync = !st.vsync;
        }
        y += bh + 5.0 * s;
        let aa = if st.msaa == 0 { "Anti-aliasing: OFF".to_string() } else { format!("Anti-aliasing: {}x", st.msaa) };
        if self.ui.button(Rect::new(left, y, half, bh), &aa, true) {
            let levels = settings::MSAA_LEVELS;
            let i = levels.iter().position(|&l| l == st.msaa).unwrap_or(2);
            self.settings.msaa = levels[(i + 1) % levels.len()];
        }
        let parts = ["Particles: All", "Particles: Fewer", "Particles: Minimal"][st.particles.min(2) as usize];
        if self.ui.button(Rect::new(right, y, half, bh), parts, true) {
            self.settings.particles = (st.particles + 1) % 3;
        }
        y += bh + 5.0 * s;
        let waving = if st.waving_leaves { "Leaves: Waving" } else { "Leaves: Still" };
        if self.ui.button(Rect::new(left, y, half, bh), waving, true) {
            self.settings.waving_leaves = !st.waving_leaves;
        }
        let shiny = if st.water_reflections { "Water: Shiny" } else { "Water: Plain" };
        if self.ui.button(Rect::new(right, y, half, bh), shiny, true) {
            self.settings.water_reflections = !st.water_reflections;
        }
        y += bh + 5.0 * s;
        let lighting = if st.smooth_lighting { "Lighting: Smooth" } else { "Lighting: Flat" };
        if self.ui.button(Rect::new(left, y, half, bh), lighting, true) {
            self.settings.smooth_lighting = !st.smooth_lighting;
        }
        let clouds = match (st.clouds, st.fancy_clouds) {
            (false, _) => "Clouds: OFF",
            (true, true) => "Clouds: Fancy",
            (true, false) => "Clouds: Fast",
        };
        if self.ui.button(Rect::new(right, y, half, bh), clouds, true) {
            // Fancy -> Fast -> Off -> Fancy.
            (self.settings.clouds, self.settings.fancy_clouds) = match (st.clouds, st.fancy_clouds) {
                (true, true) => (true, false),
                (true, false) => (false, false),
                (false, _) => (true, true),
            };
        }
        y += bh + 5.0 * s;
        if self.ui.button(Rect::new(left, y, half, bh), &on_off("View Bobbing", st.view_bobbing), true) {
            self.settings.view_bobbing = !st.view_bobbing;
        }
        if self.ui.button(Rect::new(right, y, half, bh), &on_off("Fog", st.fog), true) {
            self.settings.fog = !st.fog;
        }
        y += bh + 5.0 * s;
        if self.ui.button(Rect::new(left, y, half, bh), &on_off("Shadows", st.shadows), true) {
            self.settings.shadows = !st.shadows;
        }
        let depth = if st.fancy_water { "Water Depth: Fancy" } else { "Water Depth: Simple" };
        if self.ui.button(Rect::new(right, y, half, bh), depth, true) {
            self.settings.fancy_water = !st.fancy_water;
        }
        y += bh + 5.0 * s;
        let far = match (st.distant_terrain, st.smooth_far) {
            (false, _) => "Distant Land: OFF",
            (true, false) => "Distant Land: Blocky",
            (true, true) => "Distant Land: Smooth",
        };
        if self.ui.button(Rect::new(left, y, half, bh), far, true) {
            // Off, blocky, smooth, off...
            (self.settings.distant_terrain, self.settings.smooth_far) = match (st.distant_terrain, st.smooth_far) {
                (false, _) => (true, false),
                (true, false) => (true, true),
                (true, true) => (false, false),
            };
        }
        let (size, name) = crate::render::SHADOW_SIZES[st.shadow_quality as usize % crate::render::SHADOW_SIZES.len()];
        let short = if st.shadow_quality as usize == crate::render::SHADOW_SIZES.len() - 1 { "Why" } else { name };
        if self.ui.button(Rect::new(right, y, half, bh), &format!("Shadow Map: {short} ({size})"), st.shadows) {
            self.settings.shadow_quality = (st.shadow_quality + 1) % crate::render::SHADOW_SIZES.len() as u8;
        }
        y += bh + 4.0 * s;
        // VSync and anti-aliasing are set when the window opens.
        if (self.settings.vsync, self.settings.msaa) != self.video_at_start {
            self.ui.text_centered("VSync and anti-aliasing change next time the game starts.", w / 2.0, y + 9.0 * s, 8.0, GOLD);
        } else if st.shadows && st.shadow_quality as usize == crate::render::SHADOW_SIZES.len() - 1 {
            self.ui.text_centered(&format!("{size} shadows? {name}"), w / 2.0, y + 9.0 * s, 8.0, GOLD);
        }
        y += 16.0 * s;
        if self.ui.button(Rect::new(x, y, bw, bh), "Done", true) {
            self.set_screen(Screen::Options { from_title });
        }
    }

    /// Every action with its two bindings; click one, then press the new key.
    pub(crate) fn controls_screen(&mut self, from_title: bool) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("Controls", w / 2.0, h * 0.09, 16.0, WHITE);
        // Take the next key or button for the slot being changed (Esc empties it).
        if let Some((action, secondary)) = self.rebinding {
            if !self.rebind_armed {
                self.rebind_armed = true;
            } else if is_key_pressed(KeyCode::Escape) {
                self.settings.binds.set(action, secondary, None);
                self.rebinding = None;
            } else if let Some(b) = keybinds::Bind::just_pressed() {
                self.settings.binds.set(action, secondary, Some(b));
                self.rebinding = None;
            }
        }
        let col_w = ((w - 24.0 * s) / 2.0).min(260.0 * s);
        let bh = 15.0 * s;
        let per_col = keybinds::ACTIONS.len().div_ceil(2);
        let top = h * 0.14;
        for (i, &action) in keybinds::ACTIONS.iter().enumerate() {
            let (col, row) = (i / per_col, i % per_col);
            let x = w / 2.0 + if col == 0 { -col_w - 4.0 * s } else { 4.0 * s };
            let y = top + row as f32 * (bh + 3.0 * s);
            self.ui.text(action.label(), x, y + bh * 0.7, 9.0, WHITE);
            let binds = self.settings.binds.get(action);
            let bw = col_w * 0.28;
            for (slot, bind) in binds.iter().enumerate() {
                let r = Rect::new(x + col_w - (2 - slot) as f32 * (bw + 3.0 * s), y, bw, bh);
                let waiting = self.rebinding == Some((action, slot == 1));
                let label = if waiting { "> press <".to_string() } else { bind.map(|b| b.display()).unwrap_or_else(|| "-".into()) };
                if self.ui.button(r, &label, self.rebinding.is_none() || waiting) {
                    self.rebinding = Some((action, slot == 1));
                    self.rebind_armed = false;
                }
            }
        }
        let y = top + per_col as f32 * (bh + 3.0 * s) + 4.0 * s;
        let hint = if self.rebinding.is_some() {
            "Press a key or mouse button (Esc: leave it empty).".to_string()
        } else {
            match &self.pad.name {
                Some(name) => format!("Controller: {name} (sticks walk and look; triggers mine and place; A jumps; Y inventory)"),
                None => "No controller found. Plug one in any time.".to_string(),
            }
        };
        self.ui.text_centered(&hint, w / 2.0, y + 8.0 * s, 8.0, Color::new(0.8, 0.8, 0.8, 1.0));
        let bw = (110.0 * s).min(w * 0.4);
        let by = y + 16.0 * s;
        if self.ui.button(Rect::new(w / 2.0 - bw - 4.0 * s, by, bw, 18.0 * s), "Reset to Defaults", self.rebinding.is_none()) {
            self.settings.binds = Default::default();
        }
        if self.ui.button(Rect::new(w / 2.0 + 4.0 * s, by, bw, 18.0 * s), "Done", self.rebinding.is_none()) {
            self.set_screen(Screen::Options { from_title });
        }
    }

    pub(crate) fn world_settings_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
        self.ui.text_centered("World Settings", w / 2.0, h * 0.2, 16.0, WHITE);
        let owner = !self.game.is_client();
        let bw = (240.0 * s).min(w * 0.85);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        let mut y = h * 0.3;
        let mut rules = self.game.rules;
        // Hardcore worlds are hard, and dying costs everything: that's the point.
        let locked = rules.hardcore;
        let keep = if locked {
            "Keep Inventory: OFF (hardcore)"
        } else if rules.keep_inventory {
            "Keep Inventory: ON (dying costs nothing)"
        } else {
            "Keep Inventory: OFF (you drop everything)"
        };
        if self.ui.button(Rect::new(x, y, bw, bh), keep, owner && !locked) {
            rules.keep_inventory = !rules.keep_inventory;
        }
        y += bh + 5.0 * s;
        let diff = if locked { "Difficulty: Hard (hardcore)".to_string() } else { format!("Difficulty: {}", rules.difficulty.name()) };
        if self.ui.button(Rect::new(x, y, bw, bh), &diff, owner && !locked) {
            rules.difficulty = rules::Difficulty::from_index((rules.difficulty.index() + 1) % 4);
        }
        y += bh + 2.0 * s;
        self.ui.text_centered(rules.difficulty.blurb(), w / 2.0, y + 9.0 * s, 8.0, GRAY);
        y += 16.0 * s;
        let day = if rules.daylight_cycle { "Daylight Cycle: ON" } else { "Daylight Cycle: OFF (the sun is taking a break)" };
        if self.ui.button(Rect::new(x, y, bw, bh), day, owner) {
            rules.daylight_cycle = !rules.daylight_cycle;
        }
        y += bh + 5.0 * s;
        let half = (bw - 5.0 * s) / 2.0;
        let cycle = if rules.weather_cycle { "Weather Cycle: ON" } else { "Weather Cycle: OFF" };
        if self.ui.button(Rect::new(x, y, half, bh), cycle, owner) {
            rules.weather_cycle = !rules.weather_cycle;
        }
        use weather::Weather;
        let now = match self.game.weather.kind {
            Weather::Clear => "Weather: Clear",
            Weather::Rain => "Weather: Rain",
            Weather::Thunder => "Weather: Thunderstorm",
        };
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), now, owner) {
            let next = Weather::from_index((self.game.weather.kind.index() + 1) % 3);
            self.game.set_weather(next);
        }
        y += bh + 5.0 * s;
        let seasons = if rules.seasons { "Seasons: ON (eight days each)" } else { "Seasons: OFF" };
        if self.ui.button(Rect::new(x, y, half, bh), seasons, owner) {
            rules.seasons = !rules.seasons;
        }
        let border = if rules.border == 0 { "World Border: None".to_string() } else { format!("World Border: {} blocks", rules.border) };
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), &border, owner) {
            let i = rules::BORDERS.iter().position(|b| *b == rules.border).unwrap_or(0);
            rules.border = rules::BORDERS[(i + 1) % rules::BORDERS.len()];
        }
        if rules != self.game.rules {
            self.game.set_rules(rules);
        }
        y += bh + 8.0 * s;
        if !owner {
            self.ui.text_centered("The host decides these. You just live here.", w / 2.0, y + 6.0 * s, 9.0, GRAY);
            y += 14.0 * s;
        }
        if self.ui.button(Rect::new(x, y, bw, bh), "Done", true) {
            self.set_screen(Screen::Paused);
        }
    }
}
