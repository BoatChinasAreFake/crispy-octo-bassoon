//! The title screen, worlds, multiplayer, mods, pausing and dying.

use crate::*;

impl App {
    pub(crate) fn title_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.25));
        let px = (w / 60.0).min(h / 22.0);
        self.ui.logo(w / 2.0, h * 0.12, px);
        let pulse = 1.0 + (get_time() * 6.0).sin().abs() as f32 * 0.08;
        let size = self.ui.font(11.0 * pulse);
        let tw = measure_text(self.splash, None, size, 1.0).width;
        let (sx, sy) = (w / 2.0 + px * 16.0, h * 0.12 + px * 6.2);
        draw_text_ex(self.splash, sx - tw / 2.0, sy, TextParams { font_size: size, color: GOLD, rotation: -0.35, ..Default::default() });

        let bw = (200.0 * s).min(w * 0.8);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        let mut y = h * 0.45;
        if self.ui.button(Rect::new(x, y, bw, bh), "Singleplayer", true) {
            self.open_worlds();
            return;
        }
        y += bh + 5.0 * s;
        let half = (bw - 5.0 * s) / 2.0;
        if self.ui.button(Rect::new(x, y, half, bh), "Multiplayer", true) {
            drain_chars();
            self.set_screen(Screen::Multiplayer);
            return;
        }
        let n_mods = block::reg().mods.iter().filter(|m| m.enabled).count();
        let mods_label = if n_mods > 0 { format!("Mods ({n_mods})") } else { "Mods".to_string() };
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), &mods_label, true) {
            self.mods_scroll = 0;
            self.set_screen(Screen::Mods);
            return;
        }
        y += bh + 5.0 * s;
        let half = (bw - 5.0 * s) / 2.0;
        if self.ui.button(Rect::new(x, y, half, bh), "Options", true) {
            self.set_screen(Screen::Options { from_title: true });
        }
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), "How to Play", true) {
            self.set_screen(Screen::Help { from_title: true });
        }
        y += bh + 5.0 * s;
        if self.ui.button(Rect::new(x, y, bw, bh), "Quit to Desktop", true) {
            self.quit = true;
        }
        if let Some(tag) = self.updates.newer.clone() {
            // A newer release is out: say so, and offer the download page.
            y += bh + 10.0 * s;
            self.ui.text_centered(&format!("Minceraft {tag} is out!"), w / 2.0, y + 6.0 * s, 10.0, GOLD);
            if self.ui.button(Rect::new(x, y + 10.0 * s, bw, bh), "Get the New Version", true) {
                updates::open_in_browser(&updates::releases_page());
            }
        }
        self.ui.text(&format!("Minceraft {} (Rust, no browser)", paths::version()), 4.0 * s, h - 5.0 * s, 8.0, WHITE);
        let c = "Not affiliated with any block-game company. Please don't sue.";
        let cw = self.ui.text_width(c, 8.0);
        self.ui.text(c, w - cw - 4.0 * s, h - 5.0 * s, 8.0, WHITE);
    }

    pub(crate) fn worlds_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("Select World", w / 2.0, h * 0.07, 16.0, WHITE);
        let dir = std::fs::canonicalize(save::saves_dir()).unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join("saves"));
        self.ui.text_centered(&format!("Folder: {}", dir.display()), w / 2.0, h * 0.07 + 12.0 * s, 7.0, GRAY);
        let pw = (300.0 * s).min(w * 0.92);
        let x = w / 2.0 - pw / 2.0;
        let row_h = 30.0 * s;
        let top = h * 0.07 + 18.0 * s;
        let bottom = h - 56.0 * s;
        let rows = (((bottom - top) / row_h).floor() as usize).max(1);

        if self.worlds.is_empty() {
            self.ui.text_centered("No worlds yet. Create one below!", w / 2.0, top + 40.0 * s, 11.0, WHITE);
        }
        let wheel = mouse_wheel().1;
        if wheel.abs() > 0.1 {
            let max = self.worlds.len().saturating_sub(rows);
            self.world_scroll = if wheel > 0.0 { self.world_scroll.saturating_sub(1) } else { (self.world_scroll + 1).min(max) };
        }
        let mut play: Option<usize> = None;
        for i in self.world_scroll..(self.world_scroll + rows).min(self.worlds.len()) {
            let wd = &self.worlds[i];
            let y = top + (i - self.world_scroll) as f32 * row_h;
            let r = Rect::new(x, y, pw, row_h - 3.0 * s);
            let selected = self.world_sel == Some(i);
            let hov = self.ui.hovered(r);
            let bg = if selected { Color::new(0.25, 0.3, 0.45, 0.95) } else if hov { Color::new(0.18, 0.18, 0.22, 0.95) } else { Color::new(0.1, 0.1, 0.12, 0.9) };
            draw_rectangle(r.x, r.y, r.w, r.h, bg);
            if selected {
                draw_rectangle_lines(r.x, r.y, r.w, r.h, s, WHITE);
            }
            // A little grass (or glowrock, for creative) block as the world's icon.
            let icon = r.h - 6.0 * s;
            self.ui.icon(if wd.creative { block::GLOWROCK } else { block::GRASS }, r.x + 3.0 * s, r.y + 3.0 * s, icon);
            let tx = r.x + icon + 9.0 * s;
            let text_w = r.w - icon - 14.0 * s;
            self.ui.text(&self.ui.fit(&wd.name, 11.0, text_w), tx, r.y + 12.0 * s, 11.0, WHITE);
            let detail = match &wd.problem {
                Some(p) => format!("Can't be opened: {p}"),
                None => format!(
                    "{}  -  seed {}  -  played {}  -  {} KB",
                    if wd.creative { "Creative" } else { "Survival" },
                    wd.seed,
                    save::ago(wd.last_played),
                    wd.size.div_ceil(1024)
                ),
            };
            let col = if wd.problem.is_some() { Color::new(1.0, 0.5, 0.4, 1.0) } else { Color::new(0.75, 0.75, 0.75, 1.0) };
            self.ui.text(&self.ui.fit(&detail, 8.0, text_w), tx, r.y + 22.0 * s, 8.0, col);
            if hov && self.ui.clicked {
                // Double-click to play.
                let now = get_time();
                if self.last_click.0 == i && now - self.last_click.1 < 0.4 {
                    play = Some(i);
                }
                self.last_click = (i, now);
                self.world_sel = Some(i);
            }
        }

        let sel = self.world_sel.filter(|&i| i < self.worlds.len());
        let playable = sel.map(|i| self.worlds[i].problem.is_none()).unwrap_or(false);
        let gap = 5.0 * s;
        let bh = 20.0 * s;
        let half = (pw - gap) / 2.0;
        let quarter = (pw - 3.0 * gap) / 4.0;
        let y1 = h - 50.0 * s;
        let y2 = y1 + bh + gap;
        if self.ui.button(Rect::new(x, y1, half, bh), "Play Selected World", playable) {
            play = sel;
        }
        if self.ui.button(Rect::new(x + half + gap, y1, half, bh), "Create New World", true) {
            self.form_name = "New World".into();
            self.form_seed.clear();
            self.form_creative = false;
            self.form_keep = false;
            self.form_hardcore = false;
            self.form_focus = 0;
            drain_chars();
            self.set_screen(Screen::CreateWorld);
            return;
        }
        if self.ui.button(Rect::new(x, y2, quarter, bh), "Rename", sel.is_some()) {
            self.form_name = self.worlds[sel.unwrap()].name.clone();
            drain_chars();
            self.set_screen(Screen::RenameWorld);
            return;
        }
        if self.ui.button(Rect::new(x + quarter + gap, y2, quarter, bh), "Delete", sel.is_some()) {
            self.set_screen(Screen::DeleteWorld);
            return;
        }
        if self.ui.button(Rect::new(x + 2.0 * (quarter + gap), y2, quarter, bh), "Backups", sel.is_some()) {
            self.backup_list = backups::list(&backups::backups_dir(), &self.worlds[sel.unwrap()].id);
            self.backup_sel = if self.backup_list.is_empty() { None } else { Some(0) };
            self.set_screen(Screen::Backups);
            return;
        }
        if self.ui.button(Rect::new(x + 3.0 * (quarter + gap), y2, quarter, bh), "Back", true) {
            self.set_screen(Screen::Title);
            return;
        }
        if let Some(i) = play.filter(|&i| self.worlds.get(i).map(|w| w.problem.is_none()).unwrap_or(false)) {
            self.play_world(i);
        }
    }

    pub(crate) fn create_world_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("Create New World", w / 2.0, h * 0.14, 16.0, WHITE);
        let bw = (220.0 * s).min(w * 0.85);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        let mut y = h * 0.26;
        self.ui.text("World name", x, y - 3.0 * s, 9.0, GRAY);
        if self.ui.text_field(Rect::new(x, y, bw, bh), &self.form_name, self.form_focus == 0) {
            self.form_focus = 0;
        }
        y += bh + 14.0 * s;
        self.ui.text("Seed (leave empty for a random world; any text works)", x, y - 3.0 * s, 9.0, GRAY);
        if self.ui.text_field(Rect::new(x, y, bw, bh), &self.form_seed, self.form_focus == 1) {
            self.form_focus = 1;
        }
        y += bh + 8.0 * s;
        let mode = match (self.form_creative, self.form_hardcore) {
            (true, _) => "Game Mode: Creative",
            (false, true) => "Game Mode: Hardcore",
            (false, false) => "Game Mode: Survival",
        };
        if self.ui.button(Rect::new(x, y, bw, bh), mode, true) {
            // Survival -> Creative -> Hardcore -> Survival.
            (self.form_creative, self.form_hardcore) = match (self.form_creative, self.form_hardcore) {
                (false, false) => (true, false),
                (true, _) => (false, true),
                (false, true) => (false, false),
            };
        }
        y += bh + 4.0 * s;
        let keep = if self.form_hardcore {
            "Keep Inventory: OFF (it's hardcore)"
        } else if self.form_keep {
            "Keep Inventory: ON (dying costs nothing)"
        } else {
            "Keep Inventory: OFF (you drop everything)"
        };
        if self.ui.button(Rect::new(x, y, bw, bh), keep, !self.form_hardcore) {
            self.form_keep = !self.form_keep;
        }
        y += bh + 2.0 * s;
        let hint = if self.form_creative {
            "Fly, infinite blocks, no damage."
        } else if self.form_hardcore {
            "One life on Hard. Die, and you can only watch."
        } else {
            "Gather, craft, and try not to get hissed at."
        };
        self.ui.text_centered(hint, w / 2.0, y + 9.0 * s, 8.0, GRAY);
        y += 18.0 * s;
        let half = (bw - 5.0 * s) / 2.0;
        if self.ui.button(Rect::new(x, y, half, bh), "Create World", true) {
            self.create_world();
            return;
        }
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), "Cancel", true) {
            self.set_screen(Screen::Worlds);
        }
    }

    pub(crate) fn delete_world_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        let Some(wd) = self.world_sel.and_then(|i| self.worlds.get(i)) else {
            self.set_screen(Screen::Worlds);
            return;
        };
        let (id, name) = (wd.id.clone(), wd.name.clone());
        self.ui.text_centered(&format!("Delete \"{name}\"?"), w / 2.0, h * 0.3, 16.0, WHITE);
        self.ui.text_centered("Its builds, inventory, script data and backups will be gone for good.", w / 2.0, h * 0.3 + 20.0 * s, 10.0, Color::new(1.0, 0.6, 0.5, 1.0));
        let bw = (220.0 * s).min(w * 0.85);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        let half = (bw - 5.0 * s) / 2.0;
        let y = h * 0.45;
        if self.ui.button(Rect::new(x, y, half, bh), "Delete Forever", true) {
            match save::delete_world(&save::saves_dir(), &id).and_then(|()| backups::delete_all(&backups::backups_dir(), &id)) {
                Ok(()) => self.status = Some((format!("Deleted \"{name}\"."), 4.0)),
                Err(e) => self.status = Some((format!("Couldn't delete: {e}"), 6.0)),
            }
            self.worlds = save::list_worlds(&save::saves_dir());
            self.world_sel = if self.worlds.is_empty() { None } else { Some(0) };
            self.world_scroll = 0;
            self.set_screen(Screen::Worlds);
            return;
        }
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), "Cancel", true) {
            self.set_screen(Screen::Worlds);
        }
    }

    /// The selected world's backups: pick one and restore it as a new world.
    pub(crate) fn backups_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        let Some(wd) = self.world_sel.and_then(|i| self.worlds.get(i)) else {
            self.set_screen(Screen::Worlds);
            return;
        };
        let name = wd.name.clone();
        self.ui.text_centered(&format!("Backups of \"{name}\""), w / 2.0, h * 0.1, 16.0, WHITE);
        let note = format!("A copy is made each time you open the world; the last {} are kept. Restoring makes a new world.", backups::KEEP);
        self.ui.text_centered(&note, w / 2.0, h * 0.1 + 14.0 * s, 8.0, GRAY);
        let pw = (300.0 * s).min(w * 0.92);
        let x = w / 2.0 - pw / 2.0;
        let row_h = 24.0 * s;
        let top = h * 0.1 + 24.0 * s;
        if self.backup_list.is_empty() {
            self.ui.text_centered("No backups yet. One is made the next time you play this world.", w / 2.0, top + 30.0 * s, 10.0, WHITE);
        }
        let rows = (((h - 40.0 * s - top) / row_h).floor() as usize).max(1);
        for (i, b) in self.backup_list.iter().enumerate().take(rows) {
            let r = Rect::new(x, top + i as f32 * row_h, pw, row_h - 3.0 * s);
            let selected = self.backup_sel == Some(i);
            let hov = self.ui.hovered(r);
            let bg = if selected { Color::new(0.25, 0.3, 0.45, 0.95) } else if hov { Color::new(0.18, 0.18, 0.22, 0.95) } else { Color::new(0.1, 0.1, 0.12, 0.9) };
            draw_rectangle(r.x, r.y, r.w, r.h, bg);
            if selected {
                draw_rectangle_lines(r.x, r.y, r.w, r.h, s, WHITE);
            }
            let line = format!("{}  -  {} UTC  -  {} KB", save::ago(b.made), b.when, b.size.div_ceil(1024));
            self.ui.text(&self.ui.fit(&line, 10.0, r.w - 12.0 * s), r.x + 6.0 * s, r.y + r.h * 0.65, 10.0, WHITE);
            if hov && self.ui.clicked {
                self.backup_sel = Some(i);
            }
        }
        let bh = 20.0 * s;
        let half = (pw - 5.0 * s) / 2.0;
        let y = h - 30.0 * s;
        let chosen = self.backup_sel.filter(|&i| i < self.backup_list.len());
        if self.ui.button(Rect::new(x, y, half, bh), "Restore as a New World", chosen.is_some()) {
            let b = &self.backup_list[chosen.unwrap()];
            let root = save::saves_dir();
            match backups::restore_as_copy(&root, b, &name) {
                Ok(id) => {
                    self.worlds = save::list_worlds(&root);
                    self.world_sel = self.worlds.iter().position(|w| w.id == id);
                    self.world_scroll = 0;
                    self.status = Some((format!("Restored as \"{}\".", save::read_name(&root, &id)), 5.0));
                }
                Err(e) => self.status = Some((format!("Couldn't restore: {e}"), 6.0)),
            }
            self.set_screen(Screen::Worlds);
            return;
        }
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), "Back", true) {
            self.set_screen(Screen::Worlds);
        }
    }

    pub(crate) fn multiplayer_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("Multiplayer", w / 2.0, h * 0.11, 16.0, WHITE);
        let bw = (240.0 * s).min(w * 0.85);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        let mut y = h * 0.19;
        let masked = "*".repeat(self.mp_password.chars().count());
        let fields: [(&str, &str); 3] = [
            ("Your name", &self.mp_name),
            ("Server address: IP, IP:port, [IPv6]:port or a hostname", &self.mp_addr),
            ("Password (leave empty if the server has none)", &masked),
        ];
        let mut clicked = None;
        for (i, (label, value)) in fields.iter().enumerate() {
            self.ui.text(label, x, y - 3.0 * s, 9.0, GRAY);
            if self.ui.text_field(Rect::new(x, y, bw, bh), value, self.mp_focus == i) {
                clicked = Some(i);
            }
            y += bh + 14.0 * s;
        }
        if let Some(i) = clicked {
            self.mp_focus = i;
        }
        y -= 4.0 * s;
        let busy = self.joining.is_some() || self.connect_next.is_some();
        let label = if busy { "Connecting..." } else { "Join Server" };
        if self.ui.button(Rect::new(x, y, bw, bh), label, !busy) {
            self.connect_next = Some(self.mp_addr.clone());
        }
        y += bh + 5.0 * s;
        if self.ui.button(Rect::new(x, y, bw, bh), "Back", true) {
            self.joining = None;
            self.set_screen(Screen::Title);
        }
        y += bh + 14.0 * s;
        for line in [
            "Host from a world: Esc > \"Open to LAN\" or \"Open to Internet\". Your password above protects it.",
            "Or run a dedicated server: minceraft --server --password <pw>. Default port 25565.",
            "Tab switches fields. Enter joins.",
        ] {
            self.ui.text_centered(line, w / 2.0, y, 9.0, Color::new(0.85, 0.85, 0.85, 1.0));
            y += 12.0 * s;
        }
    }

    pub(crate) fn mods_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("Mods", w / 2.0, h * 0.08, 16.0, WHITE);
        let dir = std::fs::canonicalize(mods::mods_dir()).unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join("mods"));
        self.ui.text_centered(&format!("Folder: {}", dir.display()), w / 2.0, h * 0.08 + 14.0 * s, 8.0, GRAY);

        let infos = block::reg().mods.clone();
        let pw = (320.0 * s).min(w * 0.92);
        let x = w / 2.0 - pw / 2.0;
        let row_h = 38.0 * s;
        let top = h * 0.08 + 22.0 * s;
        let bottom = h - 34.0 * s;
        let rows = (((bottom - top) / row_h).floor() as usize).max(1);
        if infos.is_empty() {
            let lines = [
                "No mods installed yet.",
                "A mod is a folder with a mod.txt inside, placed in the folder above.",
                "Try the example: copy example-mods/cheese into mods/, then press Reload.",
                "MODDING.md explains everything a mod can add.",
            ];
            for (i, l) in lines.iter().enumerate() {
                self.ui.text_centered(l, w / 2.0, top + (20.0 + i as f32 * 13.0) * s, 10.0, WHITE);
            }
        }
        let wheel = mouse_wheel().1;
        if wheel.abs() > 0.1 {
            let max = infos.len().saturating_sub(rows);
            self.mods_scroll = if wheel > 0.0 { self.mods_scroll.saturating_sub(1) } else { (self.mods_scroll + 1).min(max) };
        }
        let mut toggle: Option<(String, bool)> = None;
        for (i, m) in infos.iter().enumerate().skip(self.mods_scroll).take(rows) {
            let y = top + (i - self.mods_scroll) as f32 * row_h;
            draw_rectangle(x, y, pw, row_h - 3.0 * s, Color::new(0.12, 0.12, 0.15, 0.95));
            draw_rectangle_lines(x, y, pw, row_h - 3.0 * s, s, if m.enabled { Color::new(0.4, 0.8, 0.4, 1.0) } else { GRAY });
            let title = format!("{}{}{}", m.name, if m.version.is_empty() { String::new() } else { format!(" v{}", m.version) }, if m.author.is_empty() { String::new() } else { format!(" by {}", m.author) });
            let text_w = pw - 75.0 * s;
            self.ui.text(&self.ui.fit(&title, 10.0, text_w), x + 5.0 * s, y + 11.0 * s, 10.0, if m.enabled { WHITE } else { GRAY });
            let detail = if !m.enabled {
                "Disabled".to_string()
            } else {
                let (b, it, r) = m.added;
                let desc = if m.description.is_empty() { String::new() } else { format!("{}  -  ", m.description) };
                let scripts = if m.scripts > 0 { format!(", {} script(s)", m.scripts) } else { String::new() };
                format!("{desc}{b} blocks, {it} items, {r} recipes{scripts}")
            };
            self.ui.text(&self.ui.fit(&detail, 8.0, text_w), x + 5.0 * s, y + 22.0 * s, 8.0, Color::new(0.8, 0.8, 0.8, 1.0));
            if let Some(e) = m.errors.first() {
                let more = if m.errors.len() > 1 { format!("  (+{} more)", m.errors.len() - 1) } else { String::new() };
                self.ui.text(&self.ui.fit(&format!("! {e}{more}"), 8.0, pw - 10.0 * s), x + 5.0 * s, y + 32.0 * s, 8.0, Color::new(1.0, 0.5, 0.4, 1.0));
            }
            let bw = 60.0 * s;
            let label = if m.enabled { "On" } else { "Off" };
            if self.ui.button(Rect::new(x + pw - bw - 5.0 * s, y + 6.0 * s, bw, 18.0 * s), label, true) {
                toggle = Some((m.id.clone(), !m.enabled));
            }
        }
        if let Some((id, on)) = toggle {
            match mods::set_enabled(&mods::mods_dir(), &id, on) {
                Ok(()) => self.reload_mods(),
                Err(e) => self.status = Some((format!("Couldn't save mod settings: {e}"), 5.0)),
            }
        }
        let bw = (120.0 * s).min(pw / 2.0 - 4.0 * s);
        let by = h - 28.0 * s;
        if self.ui.button(Rect::new(w / 2.0 - bw - 4.0 * s, by, bw, 20.0 * s), "Reload Mods", true) {
            self.reload_mods();
        }
        if self.ui.button(Rect::new(w / 2.0 + 4.0 * s, by, bw, 20.0 * s), "Done", true) {
            self.set_screen(Screen::Title);
        }
    }

    pub(crate) fn pause_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        self.ui.text_centered("Game Paused (the world politely waits)", w / 2.0, h * 0.25, 14.0, WHITE);
        let bw = (200.0 * s).min(w * 0.8);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        let mut y = h * 0.29;
        if self.ui.button(Rect::new(x, y, bw, bh), "Back to Game", true) {
            self.set_screen(Screen::Playing);
        }
        y += bh + 5.0 * s;
        let client = self.game.is_client();
        if client {
            let host = self.game.peers.get(&0).map(|p| format!("{}'s world", p.name)).unwrap_or_else(|| "a dedicated server".into());
            self.ui.text_centered(&format!("Playing on {host} ({})", players(self.game.player_count())), w / 2.0, y + bh * 0.65, 10.0, GRAY);
        } else if self.game.is_host() {
            let addr = self.lan_addr.clone().unwrap_or_default();
            let lock = if self.game.has_password() { ", password protected" } else { ", no password" };
            self.ui.text_centered(&format!("Hosting at {addr}  ({}{lock})", players(self.game.player_count())), w / 2.0, y + bh * 0.65, 10.0, GOLD);
        } else if self.ui.button(Rect::new(x, y, bw, bh), "Open to LAN", true) {
            self.host_now();
        }
        if !client {
            y += bh + 5.0 * s;
            if let Some(t) = &self.internet_status {
                // Long messages wrap onto two lines.
                let (a, b) = match t.char_indices().filter(|(_, c)| *c == ' ').map(|(i, _)| i).find(|&i| i > 60) {
                    Some(i) if t.len() > 80 => (&t[..i], &t[i + 1..]),
                    _ => (t.as_str(), ""),
                };
                self.ui.text_centered(a, w / 2.0, y + bh * 0.4, 9.0, Color::new(0.7, 0.9, 1.0, 1.0));
                self.ui.text_centered(b, w / 2.0, y + bh * 0.4 + 11.0 * s, 9.0, Color::new(0.7, 0.9, 1.0, 1.0));
            } else if self.ui.button(Rect::new(x, y, bw, bh), "Open to Internet", true) {
                self.open_to_internet();
            }
        }
        y += bh + 5.0 * s;
        let half = (bw - 5.0 * s) / 2.0;
        if self.ui.button(Rect::new(x, y, half, bh), "Save World", !client) {
            self.save();
        }
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), "World Settings", true) {
            self.set_screen(Screen::WorldSettings);
        }
        y += bh + 5.0 * s;
        let third = (bw - 10.0 * s) / 3.0;
        let adv = format!("Advancements ({}/{})", self.game.advancements.count(), advancements::ALL.len());
        if self.ui.button(Rect::new(x, y, third, bh), &self.ui.fit(&adv, 9.0, third - 6.0 * s), true) {
            self.set_screen(Screen::Advancements);
        }
        if self.ui.button(Rect::new(x + third + 5.0 * s, y, third, bh), "Statistics", true) {
            self.set_screen(Screen::Stats);
        }
        if self.ui.button(Rect::new(x + 2.0 * (third + 5.0 * s), y, third, bh), "Fishing Log", true) {
            self.set_screen(Screen::FishLog);
        }
        y += bh + 5.0 * s;
        let half = (bw - 5.0 * s) / 2.0;
        if self.ui.button(Rect::new(x, y, half, bh), "Field Journal", true) {
            self.journal_back = Screen::Paused;
            self.set_screen(Screen::Journal);
        }
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), "Beekeeping Log", true) {
            self.set_screen(Screen::BeeLog);
        }
        y += bh + 5.0 * s;
        let half = (bw - 5.0 * s) / 2.0;
        if self.ui.button(Rect::new(x, y, half, bh), "Options", true) {
            self.set_screen(Screen::Options { from_title: false });
        }
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), "How to Play", true) {
            self.set_screen(Screen::Help { from_title: false });
        }
        y += bh + 5.0 * s;
        let quit_label = if client { "Disconnect" } else { "Save and Quit to Title" };
        if self.ui.button(Rect::new(x, y, bw, bh), quit_label, true) {
            self.back_to_title();
        }
    }

    pub(crate) fn death_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.5, 0.0, 0.0, 0.45));
        let hardcore = self.game.rules.hardcore;
        self.ui.text_centered(if hardcore { "Game over!" } else { "You died! (skill issue)" }, w / 2.0, h * 0.3, 20.0, WHITE);
        if let Some(d) = &self.game.dead {
            self.ui.text_centered(d, w / 2.0, h * 0.3 + 20.0 * s, 10.0, Color::new(1.0, 0.85, 0.85, 1.0));
        }
        if let Some(p) = self.game.death_spot() {
            let line = if self.game.rules.keep_inventory || self.game.creative {
                "Your inventory is safe. This world is kind.".to_string()
            } else {
                format!("Your things are on the ground at {}, {}, {} for five minutes.", p.x, p.y, p.z)
            };
            self.ui.text_centered(&line, w / 2.0, h * 0.3 + 34.0 * s, 9.0, Color::new(1.0, 0.95, 0.7, 1.0));
        }
        let bw = (200.0 * s).min(w * 0.8);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        if hardcore {
            if self.ui.button(Rect::new(x, h * 0.5, bw, bh), "Spectate World", true) {
                self.game.spectate_after_death();
                self.set_screen(Screen::Playing);
            }
        } else if self.ui.button(Rect::new(x, h * 0.5, bw, bh), "Respawn", true) {
            self.game.respawn();
            self.set_screen(Screen::Playing);
        }
        if self.ui.button(Rect::new(x, h * 0.5 + bh + 5.0 * s, bw, bh), "Rage Quit to Title", true) {
            if hardcore {
                self.game.spectate_after_death();
            } else {
                self.game.respawn();
            }
            self.back_to_title();
        }
    }

    pub(crate) fn help_screen(&mut self, from_title: bool) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("How to Play Minceraft", w / 2.0, h * 0.1, 16.0, WHITE);
        use keybinds::Action as A;
        let k = |a: A| self.settings.binds.describe(a);
        let walk = format!("{}/{}/{}/{}", k(A::Forward), k(A::Left), k(A::Back), k(A::Right));
        let keys = [
            format!("{walk} ... walk    {} ... jump / swim up", k(A::Jump)),
            format!("{} ... sprint    {} ... sneak (won't fall off ledges)", k(A::Sprint), k(A::Sneak)),
            format!("{} ... mine / attack    {} ... place / eat / light TNT with a torch", k(A::Attack), k(A::Use)),
            format!("1-9 / wheel ... pick hotbar    {} ... pick block (creative)", k(A::PickBlock)),
            format!("{} ... inventory + crafting    {} ... throw held item (Ctrl: stack)", k(A::Inventory), k(A::Drop)),
            format!("{} ... third person    F3 ... debug info    F11 ... fullscreen    Esc ... pause", k(A::Perspective)),
        ];
        let lines = [
            keys[0].as_str(),
            keys[1].as_str(),
            keys[2].as_str(),
            keys[3].as_str(),
            keys[4].as_str(),
            keys[5].as_str(),
            "Creative: double-tap Space to fly, Shift to descend.",
            "",
            "Survival tips: punch a Tree Chunk, craft Planks, then Sticks, then a Wooden Pickaxe.",
            "Stone needs a pickaxe. Iron needs stone tier. Dimonds need iron tier. Tools wear out. Eat to heal.",
            "Hissers explode. Groaners and Rattlers burn in daylight. Bloops split. Webbers climb. Farm animals are friends (and food).",
            "Never look a Starer in the eye. Beds skip the night. Bows need Pointy Sticks. Pokey Plants poke.",
            "Farming: hoe the dirt, plant, keep it watered and lit, feed the soil, rotate crops. A Soil Probe explains.",
            "Fishing: cast, wait for the real bite (not the nibbles!), reel in. Big fish: mind the line tension.",
            "Crafting works anywhere (the table is decorative. Satire!). Chests hold things; Furnaces cook with coal or wood.",
            "Multiplayer: host opens their world with Esc > Open to LAN; friends use Multiplayer. T to chat.",
            "Change any key in Options > Controls. Game controllers work too (sticks walk and look).",
        ];
        for (i, l) in lines.iter().enumerate() {
            self.ui.text_centered(l, w / 2.0, h * 0.18 + i as f32 * 11.5 * s, 9.0, if l.is_empty() { WHITE } else { Color::new(0.9, 0.9, 0.9, 1.0) });
        }
        let bw = (160.0 * s).min(w * 0.8);
        if self.ui.button(Rect::new(w / 2.0 - bw / 2.0, h * 0.18 + 18.5 * 11.5 * s, bw, 20.0 * s), "Got it", true) {
            self.set_screen(if from_title { Screen::Title } else { Screen::Paused });
        }
    }
}
