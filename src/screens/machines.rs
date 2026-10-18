//! Machine controls share the normal immediate-mode UI and accessibility buttons.
use crate::*;
impl App {
    pub(crate) fn machine_screen(&mut self) {
        let Some(m) = self.game.machine.clone() else { return };
        let Some(spec) = machines::spec(m.block) else { return };
        let s = self.ui.s;
        let w = 300.0 * s;
        let h = (104.0 + spec.actions.len() as f32 * 38.0) * s;
        let x = (screen_width() - w) * 0.5;
        let y = ((screen_height() - h) * 0.5).max(4.0 * s);
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.55));
        draw_rectangle(x, y, w, h, ui::panel());
        draw_rectangle_lines(x, y, w, h, s, WHITE);
        let title = block::item_name(m.block);
        *self.ui.title.borrow_mut() = title.to_string();
        self.ui.text(title, x + 10.0 * s, y + 17.0 * s, 13.0, WHITE);
        self.ui.text(spec.hint, x + 10.0 * s, y + 31.0 * s, 7.0, GRAY);
        self.ui.text(&m.status, x + 10.0 * s, y + 45.0 * s, 7.0, Color::new(0.6, 0.9, 0.95, 1.0));
        draw_rectangle(x + 10.0 * s, y + 52.0 * s, w - 20.0 * s, 3.0 * s, DARKGRAY);
        draw_rectangle(x + 10.0 * s, y + 52.0 * s, (w - 20.0 * s) * m.progress as f32 / 255.0, 3.0 * s, Color::new(0.3, 0.8, 0.65, 1.0));
        for (i, a) in spec.actions.iter().enumerate() {
            let yy = y + (63.0 + i as f32 * 38.0) * s;
            let ready = a.inputs.iter().all(|(key, n)| block::reg().lookup(key).is_some_and(|id| self.game.inv.count(id) >= *n));
            if self.ui.button(Rect::new(x + 10.0 * s, yy, w - 20.0 * s, 21.0 * s), &a.label, ready) {
                self.game.machine_action(i as u8);
            }
            let cost = if a.inputs.is_empty() {
                "No ingredients".into()
            } else {
                a.inputs
                    .iter()
                    .map(|(key, n)| {
                        let id = block::reg().lookup(key).unwrap_or(0);
                        format!("{}: {}/{n}", block::item_name(id), self.game.inv.count(id))
                    })
                    .collect::<Vec<_>>()
                    .join("    ")
            };
            self.ui.text(&cost, x + 12.0 * s, yy + 31.0 * s, 7.0, if ready { GRAY } else { Color::new(1.0, 0.65, 0.45, 1.0) });
        }
        if self.ui.button(Rect::new(x + w - 76.0 * s, y + h - 27.0 * s, 66.0 * s, 19.0 * s), "Close", true) {
            self.set_screen(Screen::Playing);
        }
        self.ui.text("Esc / inventory to close", x + 10.0 * s, y + h - 14.0 * s, 7.0, GRAY);
    }
}

impl App {
    pub(crate) fn factory_screen(&mut self) {
        let Some(p) = self.game.open else { return };
        let id = self.game.world.get_v(p);
        let Some(c) = self.game.world.containers.get(&p).cloned() else { return };
        if c.slots.len() != factory::SLOTS {
            return;
        }
        let s = self.ui.s;
        let slot = 20.0 * s;
        let w = 230.0 * s;
        let h = 238.0 * s;
        let x = (screen_width() - w) * 0.5;
        let y = ((screen_height() - h) * 0.5).max(3.0 * s);
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.55));
        draw_rectangle(x, y, w, h, ui::panel());
        draw_rectangle_lines(x, y, w, h, s, WHITE);
        let title = item_name(id);
        *self.ui.title.borrow_mut() = title.to_string();
        self.ui.text(title, x + 8.0 * s, y + 15.0 * s, 11.0, WHITE);
        let v = self.game.factory_visual(p);
        let status = v.map_or("Waiting for host...", |v| v.status());
        self.ui.text(status, x + 8.0 * s, y + 28.0 * s, 8.0, Color::new(0.55, 0.9, 0.9, 1.0));
        self.ui.text(&format!("Energy: {} / 1000", v.map_or(0, |v| v.energy)), x + 8.0 * s, y + 40.0 * s, 7.0, GRAY);
        self.ui.text(&format!("Work: {}%", v.map_or(0, |v| v.progress as u32 * 100 / 255)), x + 127.0 * s, y + 40.0 * s, 7.0, GRAY);
        draw_rectangle(x + 8.0 * s, y + 45.0 * s, 105.0 * s, 3.0 * s, DARKGRAY);
        draw_rectangle(x + 8.0 * s, y + 45.0 * s, 105.0 * s * v.map_or(0.0, |v| v.energy as f32 / 1000.0), 3.0 * s, SKYBLUE);
        draw_rectangle(x + 127.0 * s, y + 45.0 * s, 95.0 * s, 3.0 * s, DARKGRAY);
        draw_rectangle(x + 127.0 * s, y + 45.0 * s, 95.0 * s * v.map_or(0.0, |v| v.progress as f32 / 255.0), 3.0 * s, GREEN);
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        let mut tooltip = None;
        for (i, dx, title) in
            [(0, 8.0, "Input"), (1, 42.0, "Fuel / coolant"), (2, 104.0, "Output"), (3, 130.0, "Byproduct"), (4, 170.0, "Power cell"), (5, 196.0, "Empty cell")]
        {
            let (l, r, hov) = self.ui.slot_worn(c.slots[i], c.wear[i], x + dx * s, y + 58.0 * s, slot, false);
            if hov {
                tooltip = label(c.slots[i], c.wear[i]).or(Some(title.to_string()));
            }
            if l || r {
                self.game.container_click(i, r, l && shift);
            }
        }
        self.ui.text("Inputs", x + 8.0 * s, y + 55.0 * s, 7.0, GRAY);
        self.ui.text("Outputs", x + 104.0 * s, y + 55.0 * s, 7.0, GRAY);
        self.ui.text("Power", x + 170.0 * s, y + 55.0 * s, 7.0, GRAY);
        for (action, label, dx) in [(0, "Run once", 8.0), (1, "Stop", 80.0), (2, if v.is_some_and(|v| v.mode == 2) { "Auto: on" } else { "Auto: off" }, 152.0)]
        {
            if self.ui.button(Rect::new(x + dx * s, y + 84.0 * s, 68.0 * s, 17.0 * s), label, true) {
                self.game.factory_action(action);
            }
        }
        self.ui.text(factory::hint(id), x + 8.0 * s, y + 105.0 * s, 7.0, GRAY);
        self.ui.text("Hoppers feed from top / sides; extract below.", x + 8.0 * s, y + 115.0 * s, 7.0, GRAY);
        let inv_y = y + 125.0 * s;
        let inv_x = x + (w - slot * 9.0) * 0.5;
        for i in 9..36 {
            let j = i - 9;
            let (l, r, hov) = self.inv_slot(i, inv_x + (j % 9) as f32 * slot, inv_y + (j / 9) as f32 * slot, slot, false);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, shift);
        }
        for i in 0..9 {
            let (l, r, hov) = self.inv_slot(i, inv_x + i as f32 * slot, inv_y + slot * 3.0 + 6.0 * s, slot, i == self.game.inv.selected);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, shift);
        }
        if let Some(cur) = self.game.inv.cursor {
            let (mx, my) = mouse_position();
            self.ui.stack_worn(Some(cur), self.game.inv.cursor_wear, mx - slot * 0.5, my - slot * 0.5, slot, true);
        } else if let Some(t) = tooltip {
            self.ui.tooltip(&t);
        }
    }
}
