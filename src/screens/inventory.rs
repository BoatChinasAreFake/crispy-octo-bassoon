//! The inventory, the recipe book, chests and the crafting table.

use crate::*;

/// The kinds of things in the creative palette.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreativeTab {
    All,
    Blocks,
    Plants,
    Tools,
    Food,
    Other,
}

impl CreativeTab {
    pub(crate) const ALL: [CreativeTab; 6] = [CreativeTab::All, CreativeTab::Blocks, CreativeTab::Plants, CreativeTab::Tools, CreativeTab::Food, CreativeTab::Other];

    pub(crate) fn name(self) -> &'static str {
        match self {
            CreativeTab::All => "Everything",
            CreativeTab::Blocks => "Building Blocks",
            CreativeTab::Plants => "Plants and Nature",
            CreativeTab::Tools => "Tools and Armour",
            CreativeTab::Food => "Food",
            CreativeTab::Other => "Everything Else",
        }
    }

    fn plant(id: Id) -> bool {
        is_block_item(id) && (matches!(block(id).model, Model::Cross) || is_leaves(id) || is_log(id) || matches!(id, GRASS | DIRT | SAND | GRAVEL | SNOW_GRASS | MUD | LILY_PAD | LEAF_LITTER | PINK_PETALS | WILDFLOWERS))
    }

    pub(crate) fn holds(self, id: Id) -> bool {
        let tool = durability(id).is_some() || armor_points(id) > 0;
        let food = food_value(id).is_some();
        let plant = Self::plant(id);
        match self {
            CreativeTab::All => true,
            CreativeTab::Blocks => is_block_item(id) && !plant,
            CreativeTab::Plants => plant,
            CreativeTab::Tools => tool,
            CreativeTab::Food => food,
            CreativeTab::Other => !is_block_item(id) && !tool && !food,
        }
    }
}

impl App {
    pub(crate) fn inventory_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        let slot = 20.0 * s;
        let creative = self.game.creative;
        let left_w = slot * 9.0 + 12.0 * s;
        let armor_w = slot + 12.0 * s;
        let right_w = (170.0 * s).min(w - left_w - armor_w - 36.0 * s);
        let total_w = armor_w + 6.0 * s + left_w + 8.0 * s + right_w;
        let panel_h = (slot * 9.5).min(h - 20.0 * s).max(slot * 7.0);
        let x0 = (w - total_w) / 2.0 + armor_w + 6.0 * s;
        let y0 = (h - panel_h) / 2.0;
        draw_rectangle(x0, y0, left_w, panel_h, ui::PANEL);
        draw_rectangle_lines(x0, y0, left_w, panel_h, s, WHITE);
        let mut tooltip: Option<String> = None;
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);

        // Armour: four slots, each taking only its own kind.
        let ax = x0 - armor_w - 6.0 * s;
        let armor_h = 18.0 * s + slot * 4.0 + 24.0 * s + 12.0 * s + slot + 4.0 * s;
        draw_rectangle(ax, y0, armor_w, armor_h, ui::PANEL);
        draw_rectangle_lines(ax, y0, armor_w, armor_h, s, WHITE);
        self.ui.text("Armour", ax + 4.0 * s, y0 + 12.0 * s, 8.0, WHITE);
        for i in 0..4 {
            let (sx, sy) = (ax + 6.0 * s, y0 + 18.0 * s + i as f32 * slot);
            let worn = self.game.inv.armor[i];
            let (l, r, hov) = self.ui.slot_worn(worn, self.game.inv.armor_wear[i], sx, sy, slot, false);
            if worn.is_none() {
                // A faint outline of what goes here.
                let pad = slot * 0.12;
                self.ui.tile(texture::T_ARMOR_ITEMS + 4 + i as u16, sx + pad, sy + pad, slot - 2.0 * pad, Color::new(0.0, 0.0, 0.0, 0.35));
            }
            if hov {
                tooltip = Some(match worn {
                    Some((id, _)) => format!("{} (+{} armour)", label(worn, self.game.inv.armor_wear[i]).unwrap_or_default(), armor_points(id)),
                    None => ["Helmet", "Chestplate", "Leggings", "Boots"][i].to_string(),
                });
            }
            if l || r {
                self.game.inv.click_armor(i);
            }
        }
        let points = self.game.inv.armor_points();
        let cut = (points as f32 * 4.0).min(80.0);
        self.ui.text(&format!("{points} pts"), ax + 4.0 * s, y0 + 18.0 * s + slot * 4.0 + 9.0 * s, 7.0, GRAY);
        self.ui.text(&format!("-{cut:.0}% dmg"), ax + 4.0 * s, y0 + 18.0 * s + slot * 4.0 + 18.0 * s, 7.0, GRAY);
        // The other hand (Swap Hands puts what you're holding there).
        let oy = y0 + 18.0 * s + slot * 4.0 + 24.0 * s;
        self.ui.text("Other hand", ax + 4.0 * s, oy + 8.0 * s, 7.0, WHITE);
        let (l, r, hov) = self.ui.slot_worn(self.game.inv.offhand, self.game.inv.offhand_wear, ax + 6.0 * s, oy + 12.0 * s, slot, false);
        if hov {
            tooltip = Some(label(self.game.inv.offhand, self.game.inv.offhand_wear).unwrap_or_else(|| "Other hand (a shield, torches...)".into()));
        }
        if l || r {
            self.game.inv.click_offhand(r);
        }

        let sx = x0 + 6.0 * s;
        let inv_top = y0 + 18.0 * s;
        // Creative: the palette, or your own inventory (the button at the top switches).
        if creative {
            let label = if self.creative_backpack { "Palette" } else { "Inventory" };
            if self.ui.button(Rect::new(x0 + left_w - 56.0 * s, y0 + 3.0 * s, 50.0 * s, 13.0 * s), label, true) {
                self.creative_backpack = !self.creative_backpack;
            }
        }
        if creative && !self.creative_backpack {
            self.ui.text("Palette", sx, y0 + 12.0 * s, 10.0, WHITE);
            let search = self.book_search.to_lowercase();
            let items: Vec<Id> = creative_items().into_iter().filter(|&id| CreativeTab::ALL[self.creative_tab as usize % CreativeTab::ALL.len()].holds(id) && (search.is_empty() || item_name(id).to_lowercase().contains(&search))).collect();
            // Only the rows that fit above the hotbar; the wheel scrolls through the rest.
            let hot_top = y0 + panel_h - slot - 6.0 * s - 12.0 * s;
            let visible = (((hot_top - inv_top) / slot).floor() as usize).max(1);
            let rows = items.len().div_ceil(9);
            let most = rows.saturating_sub(visible);
            let (mx, my) = mouse_position();
            if mx >= sx && mx < sx + slot * 9.0 && my >= inv_top && my < inv_top + visible as f32 * slot {
                let wheel = mouse_wheel().1;
                if wheel < 0.0 {
                    self.palette_scroll += 1;
                } else if wheel > 0.0 {
                    self.palette_scroll = self.palette_scroll.saturating_sub(1);
                }
            }
            if most > 0 {
                // Where in the list we are (drag the knob, or click along the bar to jump there).
                let bar_h = visible as f32 * slot;
                let knob = bar_h * visible as f32 / rows as f32;
                let bar_x = sx + slot * 9.0 + 1.0 * s;
                if self.ui.clicked && (bar_x - 3.0 * s..bar_x + 7.0 * s).contains(&mx) && (inv_top..inv_top + bar_h).contains(&my) {
                    self.palette_drag = true;
                }
                if !is_mouse_button_down(MouseButton::Left) {
                    self.palette_drag = false;
                }
                if self.palette_drag {
                    let along = ((my - inv_top - knob / 2.0) / (bar_h - knob).max(1.0)).clamp(0.0, 1.0);
                    self.palette_scroll = (along * most as f32).round() as usize;
                }
            }
            self.palette_scroll = self.palette_scroll.min(most);
            if most > 0 {
                let bar_h = visible as f32 * slot;
                let knob = bar_h * visible as f32 / rows as f32;
                let at = (bar_h - knob) * self.palette_scroll as f32 / most as f32;
                draw_rectangle(sx + slot * 9.0 + 1.0 * s, inv_top, 3.0 * s, bar_h, Color::new(0.0, 0.0, 0.0, 0.4));
                draw_rectangle(sx + slot * 9.0 + 1.0 * s, inv_top + at, 3.0 * s, knob, GRAY);
            }
            let first = self.palette_scroll * 9;
            for (i, &item) in items.iter().enumerate().skip(first).take(visible * 9) {
                let i = i - first;
                let (cx, cy) = (sx + (i % 9) as f32 * slot, inv_top + (i / 9) as f32 * slot);
                let (l, r, hov) = self.ui.slot(Some((item, 1)), cx, cy, slot, false);
                if hov {
                    tooltip = Some(item_name(item).to_string());
                }
                if l || r {
                    let n = if r { 1 } else { max_stack(item) };
                    self.game.inv.cursor = Some((item, n));
                }
            }
        } else {
            self.ui.text("Inventory", sx, y0 + 12.0 * s, 10.0, WHITE);
            let sort_x = if creative { x0 + left_w - 98.0 * s } else { x0 + left_w - 42.0 * s };
            if self.ui.button(Rect::new(sort_x, y0 + 3.0 * s, 36.0 * s, 13.0 * s), "Sort", self.game.inv.cursor.is_none()) {
                self.game.inv.sort_backpack();
            }
            for i in 9..36 {
                let j = i - 9;
                let (cx, cy) = (sx + (j % 9) as f32 * slot, inv_top + (j / 9) as f32 * slot);
                let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], cx, cy, slot, false);
                if hov {
                    tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
                }
                if self.game.bundle_click(i, l, r) {
                    continue;
                }
                if l && !(shift && self.game.inv.equip(i)) {
                    self.game.inv.click(i);
                }
                if r {
                    self.game.inv.right_click(i);
                }
            }
        }
        let hot_y = y0 + panel_h - slot - 6.0 * s;
        self.ui.text("Hotbar", sx, hot_y - 3.0 * s, 9.0, GRAY);
        if creative {
            // Three saved hotbars (kept in settings.txt): save this one, or swap one in.
            let bw = 26.0 * s;
            for k in 0..3 {
                let bx = sx + slot * 9.0 - (3 - k) as f32 * (bw * 2.0 + 3.0 * s);
                let by = hot_y - 14.0 * s;
                if self.ui.button(Rect::new(bx, by, bw, 11.0 * s), &format!("Save {}", k + 1), true) {
                    self.settings.hotbars[k] = self.game.inv.slots[..9].iter().map(|st| st.map(|(id, _)| reg().key_of(id)).unwrap_or("")).collect::<Vec<_>>().join(",");
                    self.save_settings();
                }
                let has = !self.settings.hotbars[k].is_empty();
                if self.ui.button(Rect::new(bx + bw + 1.0 * s, by, bw, 11.0 * s), &format!("Load {}", k + 1), has) {
                    let keys: Vec<String> = self.settings.hotbars[k].split(',').map(str::to_string).collect();
                    for i in 0..9 {
                        let id = keys.get(i).and_then(|key| reg().lookup(key)).filter(|&id| id != AIR);
                        self.game.inv.slots[i] = id.map(|id| (id, max_stack(id)));
                        self.game.inv.wear[i] = 0;
                    }
                }
            }
        }
        for i in 0..9 {
            let cx = sx + i as f32 * slot;
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], cx, hot_y, slot, i == self.game.inv.selected);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            if self.game.bundle_click(i, l, r) {
                continue;
            }
            if l {
                if creative && self.game.inv.cursor.is_none() {
                    self.game.inv.slots[i] = None;
                } else if !(shift && self.game.inv.equip(i)) {
                    self.game.inv.click(i);
                }
            }
            if r {
                if creative && self.game.inv.cursor.is_none() {
                    // Creative: right-click picks up one more of it, and leaves the hotbar alone.
                    self.game.inv.cursor = self.game.inv.slots[i].map(|(id, _)| (id, 1));
                } else {
                    self.game.inv.right_click(i);
                }
            }
        }
        if self.game.inv.cursor.is_some() && self.ui.clicked {
            let panels = [Rect::new(x0, y0, left_w, panel_h), Rect::new(ax, y0, armor_w, armor_h), Rect::new(x0 + left_w + 8.0 * s, y0, right_w, panel_h)];
            if !panels.iter().any(|r| self.ui.hovered(*r)) {
                // Clicked outside: creative forgets it, survival throws it on the floor.
                if creative {
                    self.game.inv.cursor = None;
                } else if let Some((item, n)) = self.game.inv.cursor.take() {
                    let wear = std::mem::take(&mut self.game.inv.cursor_wear);
                    self.game.throw_stack(item, n, wear);
                }
            }
        }

        // The recipe book (see crafting.rs), or in creative the palette's search and kinds.
        let rx = x0 + left_w + 8.0 * s;
        if !creative {
            if let Some(t) = self.recipe_book(rx, y0, right_w, panel_h) {
                tooltip = Some(t);
            }
        } else {
            self.creative_side(rx, y0, right_w, panel_h);
        }

        if let Some(c) = self.game.inv.cursor {
            let (mx, my) = mouse_position();
            self.ui.stack_worn(Some(c), self.game.inv.cursor_wear, mx - slot / 2.0, my - slot / 2.0, slot, true);
        } else if let Some(t) = tooltip {
            self.ui.tooltip(&t);
        }
    }

    /// Creative's side panel: a search box and the palette's kinds of things.
    fn creative_side(&mut self, rx: f32, y0: f32, right_w: f32, panel_h: f32) {
        let s = self.ui.s;
        draw_rectangle(rx, y0, right_w, panel_h, ui::PANEL);
        draw_rectangle_lines(rx, y0, right_w, panel_h, s, WHITE);
        self.ui.text("Find things", rx + 5.0 * s, y0 + 11.0 * s, 8.0, WHITE);
        let sb = Rect::new(rx + 4.0 * s, y0 + 15.0 * s, right_w - 8.0 * s, 12.0 * s);
        let hov_box = self.ui.hovered(sb);
        draw_rectangle(sb.x, sb.y, sb.w, sb.h, Color::new(0.05, 0.05, 0.06, 0.95));
        draw_rectangle_lines(sb.x, sb.y, sb.w, sb.h, s, if self.book_focus { Color::new(1.0, 1.0, 0.6, 1.0) } else if hov_box { WHITE } else { GRAY });
        if self.ui.clicked {
            self.book_focus = hov_box;
            if hov_box {
                drain_chars();
                self.creative_backpack = false;
            }
        }
        let caret = if self.book_focus && (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { "" };
        if self.book_search.is_empty() && !self.book_focus {
            self.ui.text("Search...", sb.x + 3.0 * s, sb.y + 9.0 * s, 7.0, GRAY);
        } else {
            self.ui.text(&format!("{}{caret}", self.book_search), sb.x + 3.0 * s, sb.y + 9.0 * s, 7.0, WHITE);
        }
        self.ui.text("Click: a stack. Right-click: one.", rx + 5.0 * s, sb.y + sb.h + 10.0 * s, 7.0, GRAY);
        let mut y = sb.y + sb.h + 16.0 * s;
        for (i, tab) in CreativeTab::ALL.iter().enumerate() {
            let on = self.creative_tab as usize == i && !self.creative_backpack;
            let label = if on { format!("> {}", tab.name()) } else { tab.name().to_string() };
            if self.ui.button(Rect::new(rx + 4.0 * s, y, right_w - 8.0 * s, 13.0 * s), &label, true) {
                self.creative_tab = i as u8;
                self.creative_backpack = false;
                self.palette_scroll = 0;
            }
            y += 15.0 * s;
        }
    }

    /// The recipe book on the right of the inventory. Returns a tooltip.
    pub(crate) fn recipe_book(&mut self, rx: f32, y0: f32, right_w: f32, panel_h: f32) -> Option<String> {
        let s = self.ui.s;
        let mut tooltip = None;
        draw_rectangle(rx, y0, right_w, panel_h, ui::PANEL);
        draw_rectangle_lines(rx, y0, right_w, panel_h, s, WHITE);
        let table = self.game.table_nearby();
        let title = if table { "Crafting Table" } else { "Pocket Crafting (small things)" };
        self.ui.text(title, rx + 5.0 * s, y0 + 11.0 * s, 8.0, if table { Color::new(1.0, 0.9, 0.6, 1.0) } else { WHITE });

        // Search box.
        let sb = Rect::new(rx + 4.0 * s, y0 + 15.0 * s, right_w - 8.0 * s, 12.0 * s);
        let hov_box = self.ui.hovered(sb);
        draw_rectangle(sb.x, sb.y, sb.w, sb.h, Color::new(0.05, 0.05, 0.06, 0.95));
        draw_rectangle_lines(sb.x, sb.y, sb.w, sb.h, s, if self.book_focus { Color::new(1.0, 1.0, 0.6, 1.0) } else if hov_box { WHITE } else { GRAY });
        if self.ui.clicked {
            self.book_focus = hov_box;
            if hov_box {
                drain_chars();
            }
        }
        let caret = if self.book_focus && (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { "" };
        if self.book_search.is_empty() && !self.book_focus {
            self.ui.text("Search recipes...", sb.x + 3.0 * s, sb.y + 9.0 * s, 7.0, GRAY);
        } else {
            self.ui.text(&format!("{}{caret}", self.book_search), sb.x + 3.0 * s, sb.y + 9.0 * s, 7.0, WHITE);
        }

        // Tabs.
        let tabs = crafting::Tab::ALL;
        let tab_w = (right_w - 8.0 * s) / tabs.len() as f32;
        let ty = sb.y + sb.h + 2.0 * s;
        for (i, t) in tabs.iter().enumerate() {
            let r = Rect::new(rx + 4.0 * s + i as f32 * tab_w, ty, tab_w - s, 10.0 * s);
            let on = self.book_tab == *t;
            let hov = self.ui.hovered(r);
            draw_rectangle(r.x, r.y, r.w, r.h, if on { Color::new(0.45, 0.4, 0.25, 0.95) } else if hov { Color::new(0.3, 0.3, 0.34, 0.95) } else { Color::new(0.18, 0.18, 0.2, 0.95) });
            self.ui.text_centered(t.name(), r.x + r.w / 2.0, r.y + 7.5 * s, 6.5, if on { WHITE } else { GRAY });
            if hov && self.ui.clicked {
                self.book_tab = *t;
                self.recipe_scroll = 0.0;
            }
        }

        // "Craftable now" and how many have been found.
        let cy = ty + 12.0 * s;
        let cb = Rect::new(rx + 4.0 * s, cy, 8.0 * s, 8.0 * s);
        draw_rectangle(cb.x, cb.y, cb.w, cb.h, Color::new(0.05, 0.05, 0.06, 0.95));
        draw_rectangle_lines(cb.x, cb.y, cb.w, cb.h, s, GRAY);
        if self.book_craftable {
            draw_rectangle(cb.x + 2.0 * s, cb.y + 2.0 * s, cb.w - 4.0 * s, cb.h - 4.0 * s, Color::new(0.5, 0.9, 0.5, 1.0));
        }
        let cl = Rect::new(cb.x, cb.y, 70.0 * s, cb.h);
        if self.ui.hovered(cl) && self.ui.clicked {
            self.book_craftable = !self.book_craftable;
            self.recipe_scroll = 0.0;
        }
        self.ui.text("Craftable now", cb.x + cb.w + 3.0 * s, cb.y + 7.0 * s, 6.5, WHITE);
        let total = recipes().len();
        let found = recipes().iter().filter(|r| crafting::discovered(r, &self.game.known)).count();
        let count = format!("{found}/{total} found");
        self.ui.text(&count, rx + right_w - self.ui.text_width(&count, 6.5) - 5.0 * s, cb.y + 7.0 * s, 6.5, GRAY);

        // The list: craftable first, then the rest.
        let row_h = 20.0 * s;
        let list_top = cy + 11.0 * s;
        let visible_rows = ((y0 + panel_h - list_top - 9.0 * s) / row_h).floor().max(1.0) as usize;
        let craftable = |g: &Game, r: &Recipe| g.inv.can_craft(r) && (table || !crafting::needs_table(r));
        let mut order: Vec<usize> = (0..total)
            .filter(|&i| {
                let r = &recipes()[i];
                crafting::discovered(r, &self.game.known)
                    && (self.book_tab == crafting::Tab::All || crafting::tab_of(r) == self.book_tab)
                    && crafting::matches(r, &self.book_search)
                    && (!self.book_craftable || craftable(&self.game, r))
            })
            .collect();
        order.sort_by_key(|&i| (!craftable(&self.game, &recipes()[i]), crafting::needs_table(&recipes()[i]) && !table));
        let max_scroll = order.len().saturating_sub(visible_rows) as f32;
        self.recipe_scroll = self.recipe_scroll.min(max_scroll);
        if self.ui.hovered(Rect::new(rx, y0, right_w, panel_h)) {
            let wheel = mouse_wheel().1;
            if wheel.abs() > 0.1 {
                self.recipe_scroll = (self.recipe_scroll - wheel.signum()).clamp(0.0, max_scroll);
            }
        }
        if order.is_empty() {
            let why = if found == 0 { "Pick something up to discover recipes." } else { "Nothing matches." };
            self.ui.text(why, rx + 6.0 * s, list_top + 10.0 * s, 7.0, GRAY);
        }
        let first = self.recipe_scroll as usize;
        let mut craft: Option<(usize, bool)> = None;
        for (row, &ri) in order.iter().skip(first).take(visible_rows).enumerate() {
            let r = &recipes()[ri];
            let big = crafting::needs_table(r);
            let ok = craftable(&self.game, r);
            let ry = list_top + row as f32 * row_h;
            let rect = Rect::new(rx + 4.0 * s, ry, right_w - 8.0 * s, row_h - 2.0 * s);
            let hov = self.ui.hovered(rect);
            let pinned = self.game.pinned == Some(ri);
            let bg = if ok && hov {
                Color::new(0.3, 0.5, 0.3, 0.95)
            } else if ok {
                Color::new(0.2, 0.32, 0.2, 0.9)
            } else if hov {
                Color::new(0.28, 0.28, 0.3, 0.9)
            } else {
                Color::new(0.2, 0.2, 0.22, 0.9)
            };
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
            if pinned {
                draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, s, Color::new(1.0, 0.85, 0.3, 1.0));
            }
            let isz = row_h - 6.0 * s;
            self.ui.stack(Some(r.output), rect.x + 2.0 * s, rect.y + 1.0 * s, isz, true);
            let mut ix = rect.x + isz + 6.0 * s;
            self.ui.text("<", ix, rect.y + rect.h * 0.65, 9.0, GRAY);
            ix += 8.0 * s;
            for &(item, n) in &r.inputs {
                let have = self.game.inv.count(item) >= n as u32;
                self.ui.icon(item, ix, rect.y + 3.0 * s, isz * 0.8);
                self.ui.text(&format!("{n}"), ix + isz * 0.8, rect.y + rect.h * 0.8, 8.0, if have { WHITE } else { Color::new(1.0, 0.4, 0.4, 1.0) });
                ix += isz * 0.8 + 12.0 * s;
            }
            if big {
                // A little table icon: this one needs a crafting table.
                let tsz = 8.0 * s;
                self.ui.icon(TABLE, rect.x + rect.w - tsz - 2.0 * s, rect.y + 2.0 * s, tsz);
                if !table {
                    draw_line(rect.x + rect.w - tsz - 2.0 * s, rect.y + 2.0 * s + tsz, rect.x + rect.w - 2.0 * s, rect.y + 2.0 * s, s, RED);
                }
            }
            if hov {
                let ins: Vec<String> = r.inputs.iter().map(|&(i, n)| format!("{n}x {}", item_name(i))).collect();
                let mut t = format!("{}x {}  <=  {}", r.output.1, item_name(r.output.0), ins.join(" + "));
                if big && !table {
                    t += "\nNeeds a Crafting Table nearby.";
                }
                t += if pinned { "\nRight-click to unpin." } else { "\nRight-click to pin to the screen." };
                tooltip = Some(t);
                if self.ui.clicked && ok {
                    craft = Some((ri, is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)));
                } else if self.ui.clicked && big && !table && self.game.inv.can_craft(r) {
                    self.game.msg("That one needs a Crafting Table. Four planks; you can make one in your pockets.");
                }
                if self.ui.rclicked {
                    self.game.pinned = if pinned { None } else { Some(ri) };
                }
            }
        }
        if let Some((ri, many)) = craft {
            self.craft_recipe(ri, many);
        }
        let hidden = total - found;
        let foot = if hidden > 0 { format!("{hidden} recipes still to discover") } else { "Every recipe discovered!".to_string() };
        self.ui.text(&foot, rx + 5.0 * s, y0 + panel_h - 3.0 * s, 6.5, GRAY);
        if max_scroll > 0.0 {
            self.ui.text("scroll", rx + right_w - 26.0 * s, y0 + panel_h - 3.0 * s, 6.5, GRAY);
        }
        tooltip
    }

    /// Craft recipe `ri` once, or as many times as possible with shift.
    pub(crate) fn craft_recipe(&mut self, ri: usize, many: bool) {
        let r = &recipes()[ri];
        if r.inputs.iter().any(|i| i.0 == BUNDLE) && !self.game.bundles_empty() {
            self.game.msg("Empty your Bundles first: a Backpack is made from an empty one.");
            return;
        }
        let times = if many { 64 } else { 1 };
        let mut made = 0u8;
        for _ in 0..times {
            if !self.game.inv.craft(r) {
                break;
            }
            made += 1;
        }
        self.game.stats.crafted += made as u64 * r.output.1 as u64;
        // Joined players: the host checks the ingredients (and the table) against its ledger.
        if self.game.is_client() && !self.game.creative && made > 0 {
            self.game.net_send_msg(net::Msg::Craft { recipe: ri as u16, times: made });
        }
        if made > 0 {
            let via_gold = r.inputs.iter().any(|&(i, _)| i == GOLD_INGOT) && r.output.0 == PICK_WOOD;
            self.game.on_crafted(r.output.0, via_gold);
            if crafting::needs_table(r) {
                self.game.advance("benchmarking");
            }
        }
    }

    /// A pinned recipe's shopping list, top right.
    pub(crate) fn pinned_recipe(&mut self) {
        let Some(ri) = self.game.pinned else { return };
        let Some(r) = recipes().get(ri) else {
            self.game.pinned = None;
            return;
        };
        let s = self.ui.s;
        let row = 11.0 * s;
        let w = 110.0 * s;
        let h = row * (r.inputs.len() as f32 + 1.0) + 6.0 * s;
        let (x, y) = (screen_width() - w - 6.0 * s, 70.0 * s);
        draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.45));
        self.ui.icon(r.output.0, x + 3.0 * s, y + 2.0 * s, 9.0 * s);
        let ready = self.game.inv.can_craft(r);
        self.ui.text(&self.ui.fit(item_name(r.output.0), 7.0, w - 16.0 * s), x + 15.0 * s, y + 9.0 * s, 7.0, if ready { Color::new(0.6, 1.0, 0.6, 1.0) } else { WHITE });
        for (i, &(item, n)) in r.inputs.iter().enumerate() {
            let yy = y + row * (i as f32 + 1.0) + 2.0 * s;
            let have = self.game.inv.count(item);
            self.ui.icon(item, x + 6.0 * s, yy, 8.0 * s);
            let col = if have >= n as u32 { Color::new(0.6, 1.0, 0.6, 1.0) } else { WHITE };
            let line = format!("{}/{} {}", have.min(999), n, item_name(item));
            self.ui.text(&self.ui.fit(&line, 6.5, w - 20.0 * s), x + 17.0 * s, yy + 7.0 * s, 6.5, col);
        }
    }

    pub(crate) fn container_screen(&mut self) {
        use containers::{FUEL, INPUT, OUTPUT};
        let Some(pos) = self.game.open else { return };
        let kind = containers::store_kind(&self.game.world, &self.game.vehicles, pos);
        let Some(c) = containers::store_ref(&self.game.world, &self.game.vehicles, pos).cloned() else { return };
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        let slot = 20.0 * s;
        // Bigger chests have more rows (and the biggest is wider; see chests.rs).
        // (A backpack's pack shows only as much as the backpack reaches.)
        let shown = if chests::is_chest(kind) { c.slots.len().min(chests::slots(kind)) } else { c.slots.len() };
        let cols = if containers::is_three_slot(kind) { 9 } else { chests::columns(shown) };
        let rows = if containers::is_three_slot(kind) { 3 } else { shown.div_ceil(cols).max(1) };
        let top_h = if containers::is_three_slot(kind) { slot * 3.2 } else { slot * rows as f32 };
        let panel_w = slot * cols as f32 + 12.0 * s;
        let panel_h = 18.0 * s + top_h + 18.0 * s + slot * 3.0 + 6.0 * s + slot + 8.0 * s;
        let x0 = (w - panel_w) / 2.0;
        let y0 = ((h - panel_h) / 2.0).max(4.0 * s);
        draw_rectangle(x0, y0, panel_w, panel_h, ui::PANEL);
        draw_rectangle_lines(x0, y0, panel_w, panel_h, s, WHITE);
        let sx = x0 + 6.0 * s;
        // Your own slots, centred under a wide chest.
        let isx = x0 + (panel_w - slot * 9.0) / 2.0;
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        let mut tooltip: Option<String> = None;
        let title = if backpacks::pack_of_key(pos).is_some() { item_name(self.game.inv.held()) } else { block(kind).name };
        self.ui.text(title, sx, y0 + 12.0 * s, 10.0, WHITE);
        let sort = Rect::new(x0 + panel_w - 42.0 * s, y0 + 3.0 * s, 36.0 * s, 13.0 * s);
        if !containers::is_three_slot(kind) && self.ui.button(sort, "Sort", true) {
            self.game.sort_container();
        }
        let top = y0 + 18.0 * s;
        // (slot index, x, y) for the container's own slots.
        let spots: Vec<(usize, f32, f32)> = if containers::is_three_slot(kind) {
            let cx = sx + slot * 2.5;
            vec![(INPUT, cx, top), (FUEL, cx, top + slot * 2.2), (OUTPUT, sx + slot * 5.5, top + slot * 1.1)]
        } else {
            (0..shown).map(|i| (i, sx + (i % cols) as f32 * slot, top + (i / cols) as f32 * slot)).collect()
        };
        for &(i, x, y) in &spots {
            let (l, r, hov) = self.ui.slot_worn(c.slots[i], c.wear[i], x, y, slot, false);
            if hov {
                tooltip = label(c.slots[i], c.wear[i]);
            }
            if l || r {
                self.game.container_click(i, r, shift && l);
            }
        }
        if kind == BREWING_STAND {
            let progress = (c.cook / potions::BREW_SECS).clamp(0.0, 1.0);
            let ax = sx + slot * 3.9;
            let fy = top + slot * 1.1;
            self.ui.tile(texture::T_ARROW_UI, ax, fy, slot * 1.2, Color::new(0.3, 0.3, 0.3, 1.0));
            self.ui.tile_part(texture::T_ARROW_UI, ax, fy, slot * 1.2, progress, false, WHITE);
            let hint = match (c.slots[INPUT], c.slots[FUEL]) {
                (None, _) => "Ingredient on top: Glowshroom, Zappy Dust, Ember Shroom, Carrot, Feather (or Hisspowder)".to_string(),
                (_, None) => "A Water Bottle (or a potion) below".to_string(),
                (Some((i, _)), Some((b, _))) if potions::brew(b, i).is_none() => format!("{} does nothing to {}.", item_name(i), item_name(b)),
                _ if c.slots[OUTPUT].is_some() => "Take the potion out first.".to_string(),
                _ => "Bubbling...".to_string(),
            };
            self.ui.text(&hint, sx + slot * 0.2, top + slot * 3.1, 7.0, GRAY);
        }
        if containers::is_furnace(kind) {
            let (burn, cook) = c.gauges();
            let (fx, fy) = (sx + slot * 2.5, top + slot * 1.1);
            self.ui.tile(texture::T_FLAME, fx, fy, slot, Color::new(0.3, 0.3, 0.3, 1.0));
            self.ui.tile_part(texture::T_FLAME, fx, fy, slot, burn, true, WHITE);
            let ax = sx + slot * 3.9;
            self.ui.tile(texture::T_ARROW_UI, ax, fy, slot * 1.2, Color::new(0.3, 0.3, 0.3, 1.0));
            self.ui.tile_part(texture::T_ARROW_UI, ax, fy, slot * 1.2, cook, false, WHITE);
            let hint = match c.slots[INPUT] {
                Some((id, _)) if containers::smelt(id).is_none() => format!("{} won't cook. It's been asked.", item_name(id)),
                Some(_) if burn <= 0.0 && c.slots[FUEL].is_none() => "Needs fuel: coal, wood, sticks...".to_string(),
                None if kind == SMOKER || kind == SMOKER_LIT => "Raw food goes on top (twice as fast)".to_string(),
                None if kind == BLAST_FURNACE || kind == BLAST_FURNACE_LIT => "Sand, cobble and the like on top (twice as fast; no food)".to_string(),
                None => "Raw food, sand or cobble goes on top".to_string(),
                _ => String::new(),
            };
            self.ui.text(&hint, sx + slot * 5.0, top + slot * 2.9, 7.0, GRAY);
        }
        let inv_y = top + top_h + 14.0 * s;
        self.ui.text("Inventory (shift-click to move stacks)", isx, inv_y - 4.0 * s, 8.0, GRAY);
        for i in 9..36 {
            let j = i - 9;
            let (cx, cy) = (isx + (j % 9) as f32 * slot, inv_y + (j / 9) as f32 * slot);
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], cx, cy, slot, false);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, shift);
        }
        let hot_y = inv_y + slot * 3.0 + 6.0 * s;
        for i in 0..9 {
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], isx + i as f32 * slot, hot_y, slot, i == self.game.inv.selected);
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

    /// A grindstone or smithing table.
    pub(crate) fn bench_screen(&mut self) {
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
        let Some((pos, bench, slots, wear)) = self.game.bench.as_ref().map(|b| (b.pos, b.bench, b.slots, b.wear)) else { return };
        self.ui.text(block(self.game.world.get_v(pos)).name, sx, y0 + 12.0 * s, 10.0, WHITE);
        let top = y0 + 20.0 * s;
        let hints: &[&str] = match bench {
            smithing::Bench::Grindstone => &["Something enchanted (or worn)", "Another of the same (optional)"],
            smithing::Bench::Smithing => &["Scorchite Upgrade Template", "Dimond tool or armour", "Scorchite Ingot"],
        };
        for (i, hint) in hints.iter().enumerate() {
            let x = sx + slot * (0.3 + i as f32 * 1.4);
            let (l, r, hov) = self.ui.slot_worn(slots[i], wear[i], x, top, slot, false);
            if hov {
                tooltip = label(slots[i], wear[i]).or(Some(hint.to_string()));
            }
            if l || r {
                self.game.bench_click(i, r);
            }
        }
        let plan = self.game.bench_plan();
        let ax = sx + slot * 4.4;
        self.ui.tile(texture::T_ARROW_UI, ax, top, slot * 1.2, if plan.is_some() { WHITE } else { Color::new(0.3, 0.3, 0.3, 1.0) });
        let out_x = sx + slot * 6.2;
        let (l, _, hov) = self.ui.slot_worn(plan.map(|(i, _, _)| (i, 1)), plan.map(|p| p.1).unwrap_or(0), out_x, top, slot, false);
        if hov {
            tooltip = plan.and_then(|(i, w, _)| label(Some((i, 1)), w));
        }
        if l {
            self.game.bench_take();
        }
        let note = match (bench, plan) {
            (smithing::Bench::Grindstone, Some((_, _, xp))) if xp > 0 => format!("Grinds off the enchantments: about {xp} experience back."),
            (smithing::Bench::Grindstone, Some(_)) => "Grinds the two into one, with a little extra life.".to_string(),
            (smithing::Bench::Grindstone, None) => "Enchanted things lose their enchantments here (and give some experience back).".to_string(),
            (smithing::Bench::Smithing, Some(_)) => "Upgrade to Scorchite (enchantments and wear are kept).".to_string(),
            (smithing::Bench::Smithing, None) => "Template, Dimond gear, Scorchite Ingot: Scorchite gear.".to_string(),
        };
        self.ui.text(&note, sx, top + slot * 1.75, 7.0, if plan.is_some() { Color::new(0.5, 1.0, 0.4, 1.0) } else { GRAY });
        let inv_y = top + top_h + 12.0 * s;
        self.ui.text("Inventory", sx, inv_y - 4.0 * s, 8.0, GRAY);
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
}
