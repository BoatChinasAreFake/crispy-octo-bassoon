//! Enchanting tables, anvils, trading, looms and books.

use crate::*;

impl App {
    pub(crate) fn enchant_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        let slot = 20.0 * s;
        let panel_w = slot * 9.0 + 12.0 * s;
        let top_h = slot * 3.4;
        let panel_h = 18.0 * s + top_h + 18.0 * s + slot * 3.0 + 6.0 * s + slot + 8.0 * s;
        let x0 = (w - panel_w) / 2.0;
        let y0 = ((h - panel_h) / 2.0).max(4.0 * s);
        draw_rectangle(x0, y0, panel_w, panel_h, ui::PANEL);
        draw_rectangle_lines(x0, y0, panel_w, panel_h, s, WHITE);
        let sx = x0 + 6.0 * s;
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        let mut tooltip: Option<String> = None;
        let Some((pos, item, wear, gold)) = self.game.enchanting.as_ref().map(|e| (e.pos, e.item, e.wear, e.gold)) else { return };
        let shelves = self.game.bookshelves(pos);
        self.ui.text(&format!("Enchanting Table ({shelves} bookshel{})", if shelves == 1 { "f" } else { "ves" }), sx, y0 + 12.0 * s, 10.0, WHITE);
        let top = y0 + 20.0 * s;
        for (i, x, stack, wr, hint) in [(0, sx, item, wear, "Something to enchant"), (1, sx + slot * 1.2, gold, 0, "Gold ingots (1 to 3)")] {
            let (l, r, hov) = self.ui.slot_worn(stack, wr, x, top + slot * 0.6, slot, false);
            if hov {
                tooltip = label(stack, wr).or(Some(hint.to_string()));
            }
            if l || r {
                self.game.enchant_click(i, r);
            }
        }
        // The three offers, cheapest first; hovering hints at what's in store.
        let offers = self.game.enchant_offers();
        let bx = sx + slot * 2.7;
        let bw = panel_w - (bx - x0) - 6.0 * s;
        let bh = slot * 0.95;
        let level = self.game.level().0;
        for choice in 0..3 {
            let y = top + choice as f32 * (bh + 2.0 * s);
            let r = Rect::new(bx, y, bw, bh);
            let Some((need, bits)) = offers.map(|o| o[choice]) else {
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.15, 0.12, 0.1, 1.0));
                continue;
            };
            let blocked = self.game.enchant_blocked(choice);
            let hover = r.contains(mouse_position().into());
            let bg = if blocked.is_some() { Color::new(0.2, 0.16, 0.14, 1.0) } else if hover { Color::new(0.45, 0.3, 0.6, 1.0) } else { Color::new(0.32, 0.22, 0.42, 1.0) };
            draw_rectangle(r.x, r.y, r.w, r.h, bg);
            draw_rectangle_lines(r.x, r.y, r.w, r.h, s, Color::new(0.6, 0.5, 0.7, 1.0));
            // Only the first enchantment is revealed (Minecraft keeps some mystery too).
            let on = item.map(|s| s.0).unwrap_or(BOOK);
            let first = enchant::Enchant::ALL.iter().find(|e| e.fits(on) && enchant::level(bits, **e) > 0).map(|e| enchant::describe_for(on, enchant::with_level(0, *e, enchant::level(bits, *e)))).unwrap_or_default();
            let color = if blocked.is_some() { GRAY } else { Color::new(0.85, 1.0, 0.6, 1.0) };
            self.ui.text(&format!("{first} . . . ?"), r.x + 4.0 * s, r.y + bh * 0.45, 8.0, color);
            let cost = choice + 1;
            let req = format!("Level {need}+   costs {cost} level{} and {cost} gold", if cost == 1 { "" } else { "s" });
            self.ui.text(&req, r.x + 4.0 * s, r.y + bh * 0.85, 7.0, if level >= need || self.game.creative { GRAY } else { Color::new(1.0, 0.4, 0.4, 1.0) });
            if hover {
                let what = enchant::Enchant::ALL.iter().find(|e| e.fits(on) && enchant::level(bits, **e) > 0).map(|e| e.name()).unwrap_or("Something");
                tooltip = Some(blocked.clone().unwrap_or_else(|| format!("{what}, and maybe more")));
                if is_mouse_button_pressed(MouseButton::Left) && self.game.inv.cursor.is_none() {
                    self.game.enchant_pick(choice);
                }
            }
        }
        if offers.is_none() {
            let hint = if item.is_some() { "That can't be enchanted (or already is)." } else { "Put in a tool, weapon or armour, and some gold." };
            self.ui.text(hint, bx + 4.0 * s, top + bh * 1.6, 8.0, GRAY);
        }
        let inv_y = top + top_h + 12.0 * s;
        self.ui.text(&format!("Inventory (level {level})"), sx, inv_y - 4.0 * s, 8.0, GRAY);
        for i in 9..36 {
            let j = i - 9;
            let (cx, cy) = (sx + (j % 9) as f32 * slot, inv_y + (j / 9) as f32 * slot);
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], cx, cy, slot, false);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, shift);
        }
        let hot_y = inv_y + slot * 3.0 + 6.0 * s;
        for i in 0..9 {
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], sx + i as f32 * slot, hot_y, slot, i == self.game.inv.selected);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, shift);
        }
        if let Some(cur) = self.game.inv.cursor {
            let (mx, my) = mouse_position();
            self.ui.stack_worn(Some(cur), self.game.inv.cursor_wear, mx - slot / 2.0, my - slot / 2.0, slot, true);
        } else if let Some(t) = tooltip {
            self.ui.tooltip(&t);
        }
    }

    pub(crate) fn anvil_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        let slot = 20.0 * s;
        let panel_w = slot * 9.0 + 12.0 * s;
        let top_h = slot * 2.2;
        let panel_h = 18.0 * s + top_h + 18.0 * s + slot * 3.0 + 6.0 * s + slot + 8.0 * s;
        let x0 = (w - panel_w) / 2.0;
        let y0 = ((h - panel_h) / 2.0).max(4.0 * s);
        draw_rectangle(x0, y0, panel_w, panel_h, ui::PANEL);
        draw_rectangle_lines(x0, y0, panel_w, panel_h, s, WHITE);
        let sx = x0 + 6.0 * s;
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        let mut tooltip: Option<String> = None;
        let Some(pos) = self.game.anvil.as_ref().map(|a| a.pos) else { return };
        self.ui.text(block(self.game.world.get_v(pos)).name, sx, y0 + 12.0 * s, 10.0, WHITE);
        let top = y0 + 20.0 * s;
        let (slots, wear) = self.game.anvil.as_ref().map(|a| (a.slots, a.wear)).unwrap_or_default();
        for (i, x) in [(0, sx + slot * 0.5), (1, sx + slot * 2.5)] {
            let (l, r, hov) = self.ui.slot_worn(slots[i], wear[i], x, top, slot, false);
            if hov {
                tooltip = label(slots[i], wear[i]).or(Some(["The worn thing", "Its material, or another one like it"][i].to_string()));
            }
            if l || r {
                self.game.anvil_click(i, r);
            }
        }
        self.ui.text("+", sx + slot * 1.75, top + slot * 0.7, 12.0, WHITE);
        let plan = self.game.anvil_plan();
        let ax = sx + slot * 4.0;
        self.ui.tile(texture::T_ARROW_UI, ax, top, slot * 1.2, if plan.is_some() { WHITE } else { Color::new(0.3, 0.3, 0.3, 1.0) });
        let out_x = sx + slot * 6.0;
        let result = plan.map(|(item, _)| Some((item, 1))).unwrap_or(None);
        let (l, _, hov) = self.ui.slot_worn(result, plan.map(|(_, r)| r.wear).unwrap_or(0), out_x, top, slot, false);
        if hov {
            tooltip = plan.and_then(|(item, r)| label(Some((item, 1)), r.wear));
        }
        if l {
            self.game.anvil_take();
        }
        if let Some((_, r)) = plan {
            let level = self.game.level().0;
            let ok = self.game.creative || level >= r.cost;
            let what = if r.book { "Enchant" } else if r.combine { "Merge" } else { "Repair" };
            let text = format!("{what} cost: {} level{}{}", r.cost, if r.cost == 1 { "" } else { "s" }, if ok { "" } else { "  (Too Expensive!)" });
            self.ui.text(&text, sx, top + slot * 1.75, 8.0, if ok { Color::new(0.5, 1.0, 0.4, 1.0) } else { Color::new(1.0, 0.4, 0.4, 1.0) });
        } else {
            self.ui.text("Worn tool on the left, what it's made of (a twin, or an enchanted book) on the right.", sx, top + slot * 1.75, 7.0, GRAY);
        }
        let inv_y = top + top_h + 12.0 * s;
        self.ui.text(&format!("Inventory (level {})", self.game.level().0), sx, inv_y - 4.0 * s, 8.0, GRAY);
        for i in 9..36 {
            let j = i - 9;
            let (cx, cy) = (sx + (j % 9) as f32 * slot, inv_y + (j / 9) as f32 * slot);
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], cx, cy, slot, false);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, shift);
        }
        let hot_y = inv_y + slot * 3.0 + 6.0 * s;
        for i in 0..9 {
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], sx + i as f32 * slot, hot_y, slot, i == self.game.inv.selected);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, shift);
        }
        if let Some(cur) = self.game.inv.cursor {
            let (mx, my) = mouse_position();
            self.ui.stack_worn(Some(cur), self.game.inv.cursor_wear, mx - slot / 2.0, my - slot / 2.0, slot, true);
        } else if let Some(t) = tooltip {
            self.ui.tooltip(&t);
        }
    }

    pub(crate) fn trade_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        let slot = 20.0 * s;
        let Some((title, list)) = self.game.trade_list() else { return };
        let row = slot * 1.1;
        let panel_w = slot * 9.0 + 12.0 * s;
        let panel_h = 24.0 * s + row * list.len() as f32 + 18.0 * s + slot * 3.0 + 6.0 * s + slot + 8.0 * s;
        let x0 = (w - panel_w) / 2.0;
        let y0 = ((h - panel_h) / 2.0).max(4.0 * s);
        draw_rectangle(x0, y0, panel_w, panel_h, ui::PANEL);
        draw_rectangle_lines(x0, y0, panel_w, panel_h, s, WHITE);
        let sx = x0 + 6.0 * s;
        let mut tooltip: Option<String> = None;
        self.ui.text(&title, sx, y0 + 12.0 * s, 10.0, WHITE);
        let top = y0 + 20.0 * s;
        let (mx, my) = mouse_position();
        for (i, t) in list.iter().enumerate() {
            let y = top + i as f32 * row;
            let r = Rect::new(sx, y, panel_w - 12.0 * s, slot);
            let afford = self.game.creative || villagers::can_afford(|id| self.game.inv.count(id), t);
            let hover = r.contains(vec2(mx, my));
            draw_rectangle(r.x, r.y, r.w, r.h, if hover && afford { Color::new(0.35, 0.3, 0.2, 1.0) } else { Color::new(0.18, 0.16, 0.14, 1.0) });
            let mut x = sx;
            for (id, n) in t.give {
                if id != AIR {
                    self.ui.stack_worn(Some((id, n)), 0, x, y, slot, false);
                    if hover && mx < x + slot && mx >= x {
                        tooltip = label(Some((id, n)), 0);
                    }
                }
                x += slot;
            }
            self.ui.tile(texture::T_ARROW_UI, sx + slot * 2.3, y, slot, if afford { WHITE } else { Color::new(0.4, 0.4, 0.4, 1.0) });
            let gx = sx + slot * 3.6;
            self.ui.stack_worn(Some(t.get), t.wear, gx, y, slot, false);
            self.ui.text(&label(Some(t.get), t.wear).unwrap_or_default(), gx + slot * 1.2, y + slot * 0.62, 7.0, if afford { WHITE } else { GRAY });
            if hover && mx >= gx {
                tooltip = label(Some(t.get), t.wear);
            }
            if hover && is_mouse_button_pressed(MouseButton::Left) && self.game.inv.cursor.is_none() {
                self.game.make_trade(i);
            }
        }
        let inv_y = top + row * list.len() as f32 + 12.0 * s;
        self.ui.text("Inventory", sx, inv_y - 4.0 * s, 8.0, GRAY);
        for i in 9..36 {
            let j = i - 9;
            let (cx, cy) = (sx + (j % 9) as f32 * slot, inv_y + (j / 9) as f32 * slot);
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], cx, cy, slot, false);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, false);
        }
        let hot_y = inv_y + slot * 3.0 + 6.0 * s;
        for i in 0..9 {
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], sx + i as f32 * slot, hot_y, slot, i == self.game.inv.selected);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, false);
        }
        if let Some(cur) = self.game.inv.cursor {
            self.ui.stack_worn(Some(cur), self.game.inv.cursor_wear, mx - slot / 2.0, my - slot / 2.0, slot, true);
        } else if let Some(t) = tooltip {
            self.ui.tooltip(&t);
        }
    }

    /// At a Loom: pattern the held banner (a pattern and a dye you have), or paint
    /// one of your banners on your shield.
    pub(crate) fn loom_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
        let held = self.game.inv.held();
        if held != BANNER && held != SHIELD {
            self.set_screen(Screen::Playing);
            return;
        }
        let (pw, ph) = (300.0 * s, 210.0 * s);
        let (x0, y0) = ((w - pw) / 2.0, (h - ph) / 2.0);
        draw_rectangle(x0, y0, pw, ph, Color::new(0.2, 0.17, 0.13, 0.95));
        // A banner preview, drawn as coloured cells.
        let preview = |design: banners::Design, px: f32, py: f32, cw: f32| {
            for y in 0..banners::H {
                for x in 0..banners::W {
                    let c = carpentry::colour_rgb(design.cell(x, y) as usize);
                    draw_rectangle(px + x as f32 * cw, py + y as f32 * cw, cw + 0.5, cw + 0.5, Color::from_rgba(c[0], c[1], c[2], 255));
                }
            }
        };
        if held == SHIELD {
            self.ui.text("Paint one of your banners on your shield", x0 + 10.0 * s, y0 + 16.0 * s, 10.0, WHITE);
            let designs: Vec<u16> = (0..self.game.inv.slots.len()).filter(|&i| self.game.inv.slots[i].is_some_and(|st| st.0 == BANNER)).map(|i| enchant::enchants(self.game.inv.wear[i])).collect();
            if designs.is_empty() {
                self.ui.text("You don't have any banners with you.", x0 + 10.0 * s, y0 + 40.0 * s, 9.0, GRAY);
            }
            for (k, d) in designs.iter().take(8).enumerate() {
                let bx = x0 + 10.0 * s + k as f32 * 36.0 * s;
                preview(banners::Design::from_bits(*d), bx, y0 + 34.0 * s, 4.0 * s);
                if self.ui.button(Rect::new(bx, y0 + 86.0 * s, 32.0 * s, 16.0 * s), "Use", true) {
                    self.settings.shield_banner = *d;
                    self.game.shield_banner = *d;
                    self.save_settings();
                    self.game.advance("loomed");
                }
            }
            if self.ui.button(Rect::new(x0 + 10.0 * s, y0 + 112.0 * s, 100.0 * s, 18.0 * s), "Plain shield", true) {
                self.settings.shield_banner = 0;
                self.game.shield_banner = 0;
                self.save_settings();
            }
        } else {
            let design = banners::Design::from_bits(enchant::enchants(self.game.inv.wear[self.game.inv.selected]));
            self.ui.text(&design.describe(), x0 + 10.0 * s, y0 + 16.0 * s, 9.0, WHITE);
            preview(design, x0 + pw - 60.0 * s, y0 + 30.0 * s, 6.0 * s);
            self.ui.text("Pattern", x0 + 10.0 * s, y0 + 36.0 * s, 9.0, GRAY);
            for (k, name) in banners::PATTERNS.iter().enumerate() {
                let r = Rect::new(x0 + 10.0 * s + (k % 4) as f32 * 52.0 * s, y0 + 42.0 * s + (k / 4) as f32 * 20.0 * s, 50.0 * s, 18.0 * s);
                let on = self.loom_pattern == k as u8;
                let label = if on { format!("[{name}]") } else { name.to_string() };
                if self.ui.button(r, &label, true) {
                    self.loom_pattern = k as u8;
                }
            }
            self.ui.text("Dye", x0 + 10.0 * s, y0 + 96.0 * s, 9.0, GRAY);
            for c in 0..8u8 {
                let have = self.game.creative || self.game.inv.count(DYE_FIRST + c as Id) > 0;
                let r = Rect::new(x0 + 10.0 * s + c as f32 * 26.0 * s, y0 + 102.0 * s, 22.0 * s, 22.0 * s);
                let rgb = carpentry::colour_rgb(c as usize);
                draw_rectangle(r.x, r.y, r.w, r.h, Color::from_rgba(rgb[0], rgb[1], rgb[2], if have { 255 } else { 60 }));
                if self.loom_colour == c {
                    draw_rectangle_lines(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0, 2.0 * s, WHITE);
                }
                if have && self.ui.hovered(r) && self.ui.clicked {
                    self.loom_colour = c;
                }
            }
            let next = design.with(self.loom_pattern, self.loom_colour);
            self.ui.text("Becomes", x0 + pw - 60.0 * s, y0 + 116.0 * s, 8.0, GRAY);
            preview(next, x0 + pw - 60.0 * s, y0 + 122.0 * s, 3.5 * s);
            if self.ui.button(Rect::new(x0 + 10.0 * s, y0 + 136.0 * s, 100.0 * s, 20.0 * s), "Apply", true) && !self.game.loom_apply(self.loom_pattern, self.loom_colour) {
                self.status = Some(("You need that dye.".into(), 2.0));
            }
        }
        if self.ui.button(Rect::new(x0 + pw - 70.0 * s, y0 + ph - 26.0 * s, 60.0 * s, 20.0 * s), "Done", true) {
            self.set_screen(Screen::Playing);
        }
    }

    /// A book open on screen: read it, or (a Book and Quill) write in it and sign it.
    pub(crate) fn book_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
        let Some(view) = self.game.reading.clone() else {
            self.set_screen(Screen::Playing);
            return;
        };
        let writing = view.writing.is_some();
        let (bw, bh) = (230.0 * s, 260.0 * s);
        let (x0, y0) = ((w - bw) / 2.0, (h - bh) / 2.0 - 16.0 * s);
        draw_rectangle(x0, y0, bw, bh, Color::new(0.93, 0.89, 0.78, 1.0));
        draw_rectangle_lines(x0, y0, bw, bh, 2.0 * s, Color::new(0.45, 0.3, 0.18, 1.0));
        let ink = Color::new(0.12, 0.08, 0.05, 1.0);
        if let Some(title) = &self.book_title {
            // Signing: a title, then it's done for good.
            self.ui.text_centered("Give it a title (Enter: sign, Esc: back)", w / 2.0, y0 + 20.0 * s, 9.0, ink);
            let cursor = if (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { "" };
            self.ui.text_centered(&format!("{title}{cursor}"), w / 2.0, y0 + 60.0 * s, 12.0, ink);
            self.ui.text_centered(&format!("by {}", self.game.player_name), w / 2.0, y0 + 84.0 * s, 9.0, Color::new(0.35, 0.28, 0.2, 1.0));
            self.ui.text_centered("Once signed, nobody can change it.", w / 2.0, y0 + 120.0 * s, 8.0, Color::new(0.45, 0.35, 0.25, 1.0));
            if self.ui.button(Rect::new(w / 2.0 - 50.0 * s, y0 + bh + 8.0 * s, 100.0 * s, 20.0 * s), "Sign", true) {
                self.sign_book();
            }
            return;
        }
        let pages = self.book_pages.len();
        if !view.book.title.is_empty() && !writing {
            self.ui.text_centered(&format!("{} by {}", view.book.title, view.book.author), w / 2.0, y0 - 8.0 * s, 9.0, WHITE);
        }
        self.ui.text_centered(&format!("Page {} of {}", self.book_page + 1, pages), w / 2.0, y0 + 14.0 * s, 8.0, Color::new(0.4, 0.3, 0.2, 1.0));
        // The page, wrapped to the paper.
        let per_line = 30;
        let mut lines: Vec<String> = Vec::new();
        for para in self.book_pages[self.book_page].split('\n') {
            let chars: Vec<char> = para.chars().collect();
            if chars.is_empty() {
                lines.push(String::new());
            }
            for chunk in chars.chunks(per_line) {
                lines.push(chunk.iter().collect());
            }
        }
        if writing && (get_time() * 2.0) as i64 % 2 == 0
            && let Some(last) = lines.last_mut()
        {
            last.push('_');
        }
        for (i, l) in lines.iter().take(14).enumerate() {
            self.ui.text(l, x0 + 12.0 * s, y0 + 34.0 * s + i as f32 * 15.0 * s, 9.5, ink);
        }
        let by = y0 + bh + 8.0 * s;
        let bwid = 52.0 * s;
        if self.ui.button(Rect::new(x0, by, bwid, 20.0 * s), "< Back", self.book_page > 0) {
            self.book_page -= 1;
        }
        let can_next = self.book_page + 1 < pages || (writing && pages < books::MAX_PAGES);
        if self.ui.button(Rect::new(x0 + bw - bwid, by, bwid, 20.0 * s), "Next >", can_next) {
            if self.book_page + 1 == pages {
                self.book_pages.push(String::new());
            }
            self.book_page += 1;
        }
        let mid = Rect::new(w / 2.0 - 54.0 * s, by, 52.0 * s, 20.0 * s);
        let right = Rect::new(w / 2.0 + 2.0 * s, by, 52.0 * s, 20.0 * s);
        if writing {
            if self.ui.button(mid, "Sign", true) {
                self.book_title = Some(String::new());
                drain_chars();
            }
        } else if let Some(p) = view.lectern
            && self.ui.button(mid, "Take", true)
        {
            self.game.take_from_lectern(p);
            self.set_screen(Screen::Playing);
            return;
        }
        if self.ui.button(right, "Done", true) {
            self.set_screen(Screen::Playing);
        }
    }
}
