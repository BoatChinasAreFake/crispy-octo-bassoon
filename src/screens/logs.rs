//! Advancements, statistics and the logs: the Field Journal, the fishing and bee logs.

use crate::*;

impl App {
    pub(crate) fn advancements_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
        let title = format!("Advancements: {}/{} (they're per world, like memories)", self.game.advancements.count(), advancements::ALL.len());
        self.ui.text_centered(&title, w / 2.0, h * 0.06, 14.0, WHITE);
        // Tabs, each with how many are done.
        let tab_w = ((w - 20.0 * s) / advancements::TABS.len() as f32).min(118.0 * s);
        let tabs_x = w / 2.0 - tab_w * advancements::TABS.len() as f32 / 2.0;
        for (t, name) in advancements::TABS.iter().enumerate() {
            let on: Vec<&advancements::Advancement> = advancements::ALL.iter().filter(|a| t == 0 || advancements::tab_of(a.key) == t).collect();
            let done = on.iter().filter(|a| self.game.advancements.has(a.key)).count();
            let label = self.ui.fit(&format!("{name} {done}/{}", on.len()), 8.0, tab_w - 8.0 * s);
            let r = Rect::new(tabs_x + t as f32 * tab_w + 1.0 * s, h * 0.085, tab_w - 2.0 * s, 15.0 * s);
            if self.ui.button(r, &label, self.adv_tab != t) {
                self.adv_tab = t;
                self.adv_scroll = 0;
            }
        }
        let shown: Vec<&advancements::Advancement> = advancements::ALL.iter().filter(|a| self.adv_tab == 0 || advancements::tab_of(a.key) == self.adv_tab).collect();
        let p = &self.game.advancements;
        let cols = if w >= 2.0 * 220.0 * s { 2 } else { 1 };
        let col_w = ((w - 20.0 * s) / cols as f32).min(290.0 * s);
        let row_h = 21.0 * s;
        let per_col = shown.len().div_ceil(cols);
        let x0 = w / 2.0 - col_w * cols as f32 / 2.0;
        let y0 = h * 0.085 + 19.0 * s;
        let max_rows = ((h * 0.84 - y0) / row_h).floor().max(1.0) as usize;
        let max_scroll = per_col.saturating_sub(max_rows);
        let wheel = mouse_wheel().1;
        if wheel.abs() > 0.1 {
            self.adv_scroll = if wheel > 0.0 { self.adv_scroll.saturating_sub(1) } else { self.adv_scroll + 1 };
        }
        self.adv_scroll = self.adv_scroll.min(max_scroll);
        for (i, a) in shown.iter().enumerate() {
            let (c, r) = (i / per_col, i % per_col);
            let Some(r) = r.checked_sub(self.adv_scroll).filter(|&r| r < max_rows) else { continue };
            let (x, y) = (x0 + c as f32 * col_w, y0 + r as f32 * row_h);
            let got = p.has(a.key);
            draw_rectangle(x + 2.0 * s, y, col_w - 4.0 * s, row_h - 2.0 * s, if got { Color::new(0.25, 0.22, 0.08, 0.9) } else { Color::new(0.12, 0.12, 0.14, 0.85) });
            let tint = if got { WHITE } else { Color::new(0.3, 0.3, 0.3, 1.0) };
            self.ui.tile(texture::T_TROPHY, x + 4.0 * s, y + 1.0 * s, row_h - 4.0 * s, tint);
            let tx = x + row_h + 4.0 * s;
            let max = col_w - row_h - 10.0 * s;
            self.ui.text(&self.ui.fit(a.title, 9.0, max), tx, y + 9.0 * s, 9.0, if got { GOLD } else { GRAY });
            self.ui.text(&self.ui.fit(a.desc, 7.0, max), tx, y + 17.0 * s, 7.0, if got { WHITE } else { Color::new(0.55, 0.55, 0.55, 1.0) });
        }
        if max_scroll > 0 {
            self.ui.text_centered("(scroll for more)", w / 2.0, y0 + max_rows as f32 * row_h + 6.0 * s, 7.0, GRAY);
        }
        let bw = (160.0 * s).min(w * 0.8);
        if self.ui.button(Rect::new(w / 2.0 - bw / 2.0, h * 0.88, bw, 20.0 * s), "Done", true) {
            self.set_screen(Screen::Paused);
        }
    }

    pub(crate) fn stats_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
        self.ui.text_centered("Statistics", w / 2.0, h * 0.1, 16.0, WHITE);
        let sub = format!("{} mode{}", self.game.mode().name(), if self.game.rules.hardcore { ", hardcore" } else { "" });
        self.ui.text_centered(&sub, w / 2.0, h * 0.1 + 16.0 * s, 9.0, GOLD);
        let lines = self.game.stats.lines();
        let cols = if w >= 2.0 * 200.0 * s { 2 } else { 1 };
        let col_w = ((w - 30.0 * s) / cols as f32).min(220.0 * s);
        let row_h = 16.0 * s;
        let per_col = lines.len().div_ceil(cols);
        let x0 = w / 2.0 - (col_w * cols as f32 + 10.0 * s * (cols - 1) as f32) / 2.0;
        let y0 = h * 0.1 + 30.0 * s;
        for (i, (label, value)) in lines.iter().enumerate() {
            let (c, r) = (i / per_col, i % per_col);
            let (x, y) = (x0 + c as f32 * (col_w + 10.0 * s), y0 + r as f32 * row_h);
            if y > h * 0.84 {
                continue;
            }
            draw_rectangle(x, y, col_w, row_h - 2.0 * s, Color::new(0.12, 0.14, 0.2, 0.9));
            self.ui.text(label, x + 5.0 * s, y + 10.5 * s, 9.0, WHITE);
            let vw = self.ui.text_width(value, 9.0);
            self.ui.text(value, x + col_w - vw - 5.0 * s, y + 10.5 * s, 9.0, GOLD);
        }
        let bw = (160.0 * s).min(w * 0.8);
        if self.ui.button(Rect::new(w / 2.0 - bw / 2.0, h * 0.88, bw, 20.0 * s), "Done", true) {
            self.set_screen(Screen::Paused);
        }
    }

    /// The Field Journal: every culture's collection, and how you're doing.
    pub(crate) fn journal_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.08, 0.06, 0.03, 0.85));
        let j = &self.game.journal;
        self.ui.text_centered("Field Journal", w / 2.0, h * 0.08, 16.0, Color::new(1.0, 0.92, 0.75, 1.0));
        let sub = format!("Archaeologist level {}  ({} xp)  -  {} digs, {} cracked, {} shattered, {} restored", j.level(), j.xp, j.digs, j.cracked, j.shattered, j.restored);
        self.ui.text_centered(&sub, w / 2.0, h * 0.08 + 15.0 * s, 8.0, GOLD);
        let pw = (360.0 * s).min(w - 20.0 * s);
        let x = w / 2.0 - pw / 2.0;
        let mut y = h * 0.08 + 26.0 * s;
        let icon = 15.0 * s;
        let mut tooltip = None;
        for c in archaeology::Culture::ALL {
            let (found, total) = j.progress(c);
            let done = found == total;
            let block_h = 13.0 * s + icon + 6.0 * s;
            draw_rectangle(x, y, pw, block_h, Color::new(0.2, 0.15, 0.1, 0.9));
            if done {
                draw_rectangle_lines(x, y, pw, block_h, s, GOLD);
            }
            self.ui.text(&format!("{}  ({found}/{total})", c.title()), x + 5.0 * s, y + 10.0 * s, 9.0, if done { GOLD } else { WHITE });
            let where_ = format!("{} - {}", c.site().name(), if done { "collection complete!" } else { "keep digging" });
            let ww = self.ui.text_width(&where_, 7.0);
            self.ui.text(&where_, x + pw - ww - 5.0 * s, y + 10.0 * s, 7.0, GRAY);
            let mut ix = x + 5.0 * s;
            let iy = y + 13.0 * s;
            let items: Vec<Id> = c.shards().iter().map(|&i| SHARD_FIRST + i as Id).chain(c.relics().iter().map(|&i| RELIC_FIRST + i as Id)).collect();
            for id in items {
                let got = j.has(id);
                let r = Rect::new(ix, iy, icon, icon);
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.35));
                if got.is_some() {
                    self.ui.icon(id, ix + s, iy + s, icon - 2.0 * s);
                } else {
                    self.ui.text_centered("?", ix + icon / 2.0, iy + icon * 0.72, 9.0, Color::new(0.5, 0.45, 0.4, 1.0));
                }
                if self.ui.hovered(r) {
                    tooltip = Some(match got {
                        Some((n, cond)) if archaeology::is_relic(id) => format!("{}\nFound {n}, best: {}", item_name(id), archaeology::CONDITIONS[cond as usize]),
                        Some((n, _)) => format!("{}\nFound {n}", item_name(id)),
                        None => "Not found yet".to_string(),
                    });
                }
                ix += icon + 3.0 * s;
            }
            y += block_h + 4.0 * s;
        }
        let tips = "Brush, don't dig. Ease off when the find shifts. Deeper layers hide older, rarer things.";
        self.ui.text_centered(tips, w / 2.0, h * 0.84, 8.0, Color::new(1.0, 0.9, 0.7, 1.0));
        let bw = (160.0 * s).min(w * 0.8);
        if self.ui.button(Rect::new(w / 2.0 - bw / 2.0, h * 0.88, bw, 20.0 * s), "Done", true) {
            self.set_screen(self.journal_back);
        }
        if let Some(t) = tooltip {
            self.ui.tooltip(&t);
        }
    }

    pub(crate) fn fish_log_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
        let log = &self.game.fish_log;
        self.ui.text_centered("Fishing Log", w / 2.0, h * 0.1, 16.0, WHITE);
        let sub = format!("Angler level {}  ({} xp)  -  lines snapped: {}  (they were THIS big)", log.level(), log.xp, log.snapped);
        self.ui.text_centered(&sub, w / 2.0, h * 0.1 + 16.0 * s, 9.0, GOLD);
        let pw = (320.0 * s).min(w - 20.0 * s);
        let x = w / 2.0 - pw / 2.0;
        let mut y = h * 0.1 + 30.0 * s;
        if log.species.is_empty() {
            self.ui.text_centered("Nothing yet. Craft a Fishing Stick (3 sticks, 2 string) and find some water.", w / 2.0, y + 12.0 * s, 9.0, GRAY);
        }
        let row_h = 20.0 * s;
        for (key, (count, best)) in &log.species {
            if y > h * 0.8 {
                break;
            }
            let id = block::reg().lookup(key).unwrap_or(AIR);
            draw_rectangle(x, y, pw, row_h - 2.0 * s, Color::new(0.12, 0.14, 0.2, 0.9));
            self.ui.icon(id, x + 3.0 * s, y + 1.0 * s, row_h - 4.0 * s);
            self.ui.text(&self.ui.fit(item_name(id), 9.0, pw * 0.55), x + row_h + 4.0 * s, y + 13.0 * s, 9.0, WHITE);
            let stats = if *best > 0.0 { format!("x{count}   best {best:.0}cm") } else { format!("x{count}") };
            let sw = self.ui.text_width(&stats, 9.0);
            self.ui.text(&stats, x + pw - sw - 6.0 * s, y + 13.0 * s, 9.0, GOLD);
            y += row_h;
        }
        let tips = "Tips: dawn and dusk are best. Deep, wide water beats puddles. Worms help. Don't reel in on a nibble.";
        self.ui.text_centered(tips, w / 2.0, h * 0.84, 8.0, Color::new(0.7, 0.85, 1.0, 1.0));
        let bw = (160.0 * s).min(w * 0.8);
        if self.ui.button(Rect::new(w / 2.0 - bw / 2.0, h * 0.88, bw, 20.0 * s), "Done", true) {
            self.set_screen(Screen::Paused);
        }
    }

    /// The Beekeeping Log.
    pub(crate) fn bee_log_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.1, 0.08, 0.0, 0.8));
        let log = &self.game.bee_log;
        self.ui.text_centered("Beekeeping Log", w / 2.0, h * 0.1, 16.0, Color::new(1.0, 0.85, 0.3, 1.0));
        let sub = format!("Beekeeper level {}  ({} xp)  -  stung {} time{}", log.level(), log.xp, log.stings, if log.stings == 1 { "" } else { "s" });
        self.ui.text_centered(&sub, w / 2.0, h * 0.1 + 16.0 * s, 9.0, GOLD);
        let pw = (300.0 * s).min(w - 20.0 * s);
        let x = w / 2.0 - pw / 2.0;
        let mut y = h * 0.1 + 30.0 * s;
        let row_h = 18.0 * s;
        let mut rows: Vec<(Id, String, String)> = bees::Flavour::ALL.iter().map(|f| (f.item(), format!("{} Honey", f.name()), format!("{} bottle{}", log.bottles[f.index()], if log.bottles[f.index()] == 1 { "" } else { "s" }))).collect();
        rows.push((HONEYCOMB, "Honeycomb".into(), format!("{}", log.comb)));
        rows.push((QUEEN_BEE, "Queens caught".into(), format!("{}", log.queens)));
        rows.push((BEEHIVE, "Colonies started".into(), format!("{}", log.colonies_started)));
        rows.push((BEEHIVE_BUSY, "Swarms caught / lost".into(), format!("{} / {}", log.swarms_caught, log.swarms_lost)));
        let hives = self.game.hives.values().filter(|c| c.bees > 0).count();
        rows.push((BEE_NEST, "Colonies known nearby".into(), format!("{hives}")));
        for (id, name, val) in rows {
            draw_rectangle(x, y, pw, row_h - 2.0 * s, Color::new(0.2, 0.16, 0.05, 0.9));
            self.ui.icon(id, x + 3.0 * s, y + 1.0 * s, row_h - 4.0 * s);
            self.ui.text(&name, x + row_h + 4.0 * s, y + 12.0 * s, 9.0, WHITE);
            let vw = self.ui.text_width(&val, 9.0);
            self.ui.text(&val, x + pw - vw - 6.0 * s, y + 12.0 * s, 9.0, GOLD);
            y += row_h;
        }
        let tips = "Tips: mixed flowers make better honey. Smoke before you harvest. Wood Ash for mites. Keep a spare hive for swarms.";
        self.ui.text_centered(tips, w / 2.0, h * 0.84, 8.0, Color::new(1.0, 0.9, 0.5, 1.0));
        let bw = (160.0 * s).min(w * 0.8);
        if self.ui.button(Rect::new(w / 2.0 - bw / 2.0, h * 0.88, bw, 20.0 * s), "Done", true) {
            self.set_screen(Screen::Paused);
        }
    }
}
