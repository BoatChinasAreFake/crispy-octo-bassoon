//! What's drawn over the world while playing: the HUD, maps and compasses, boss bars and name tags.

use crate::*;

impl App {
    pub(crate) fn hud(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        let g = &self.game;
        if !g.menu && g.player.head_in_water(&g.world) {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.1, 0.2, 0.6, 0.35));
        }
        if g.player.hurt > 0.0 {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.8, 0.0, 0.0, g.player.hurt * 0.5));
        }
        if !g.menu && (g.on_fire > 0.0 || g.player.body.in_lava) {
            // On fire: an orange haze and flames licking up the edges.
            draw_rectangle(0.0, 0.0, w, h, Color::new(1.0, 0.45, 0.05, if g.player.body.in_lava { 0.55 } else { 0.18 }));
            let size = 48.0 * s;
            let flick = (g.clock * 9.0).sin() * 4.0 * s;
            for i in 0..((w / size) as i32 + 1) {
                let x = i as f32 * size;
                let lift = if i % 2 == 0 { flick } else { -flick };
                self.ui.tile(texture::T_FLAME, x, h - size * 1.4 + lift, size * 1.2, Color::new(1.0, 1.0, 1.0, 0.8));
            }
        }
        if !g.ready {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.1, 0.07, 0.05, 1.0));
            self.ui.text_centered("Generating terrain (artisanally)...", w / 2.0, h / 2.0, 12.0, WHITE);
            self.ui.text_centered(&format!("{} chunks ready", self.renderer.chunks.len()), w / 2.0, h / 2.0 + 18.0 * s, 9.0, GRAY);
            return;
        }
        self.sign_text();
        if self.screen == Screen::Playing {
            self.ui.crosshair();
            // The weapon charging back up after a swing (see combat.rs).
            let charge = self.game.attack_charge();
            if charge < 1.0 && !self.game.creative {
                let (bw, bh) = (18.0 * s, 2.0 * s);
                let (bx, by) = (w / 2.0 - bw / 2.0, h / 2.0 + 10.0 * s);
                draw_rectangle(bx - 1.0, by - 1.0, bw + 2.0, bh + 2.0, Color::new(0.0, 0.0, 0.0, 0.6));
                draw_rectangle(bx, by, bw * charge, bh, Color::new(0.9, 0.9, 0.9, 0.9));
            }
            if self.game.blocking {
                self.ui.tile(texture::T_SHIELD, w / 2.0 + 10.0 * s, h / 2.0 - 8.0 * s, 16.0 * s, Color::new(1.0, 1.0, 1.0, 0.8));
            }
        }
        self.name_tags();

        // Hotbar, hearts and hunger (spectators have no hands, and nothing to lose).
        if g.spectator {
            self.ui.text_centered("Spectating  -  fly with Jump/Sneak, sprint to go faster", w / 2.0, h - 12.0 * s, 9.0, Color::new(0.8, 0.9, 1.0, 0.8));
        } else {
            let slot = 20.0 * s;
            let x0 = w / 2.0 - slot * 4.5;
            let y0 = h - slot - 4.0 * s;
            draw_rectangle(x0 - 2.0 * s, y0 - 2.0 * s, slot * 9.0 + 4.0 * s, slot + 4.0 * s, Color::new(0.0, 0.0, 0.0, 0.45));
            for i in 0..9 {
                let x = x0 + i as f32 * slot;
                draw_rectangle_lines(x, y0, slot, slot, s, Color::new(0.6, 0.6, 0.6, 0.6));
                self.ui.stack_worn(g.inv.slots[i], g.inv.wear[i], x, y0, slot, !g.creative);
            }
            let sel = x0 + g.inv.selected as f32 * slot;
            draw_rectangle_lines(sel - s, y0 - s, slot + 2.0 * s, slot + 2.0 * s, 2.0 * s, WHITE);
            // The other hand, off to the left.
            if g.inv.offhand.is_some() {
                let ox = x0 - slot - 8.0 * s;
                draw_rectangle(ox - 2.0 * s, y0 - 2.0 * s, slot + 4.0 * s, slot + 4.0 * s, Color::new(0.0, 0.0, 0.0, 0.45));
                draw_rectangle_lines(ox, y0, slot, slot, s, Color::new(0.6, 0.6, 0.6, 0.6));
                self.ui.stack_worn(g.inv.offhand, g.inv.offhand_wear, ox, y0, slot, !g.creative);
            }
            if !g.creative {
                // Experience bar just above the hotbar, with the level in the middle.
                let (level, progress) = g.level();
                let (bw, by) = (slot * 9.0, y0 - 6.0 * s);
                draw_rectangle(x0, by, bw, 4.0 * s, Color::new(0.0, 0.0, 0.0, 0.6));
                draw_rectangle(x0, by + s, bw * progress, 2.0 * s, Color::new(0.5, 0.95, 0.2, 1.0));
                if level > 0 {
                    let t = level.to_string();
                    self.ui.text_centered(&t, x0 + bw / 2.0 + s, by + 2.0 * s, 10.0, BLACK);
                    self.ui.text_centered(&t, x0 + bw / 2.0, by + s, 10.0, Color::new(0.55, 1.0, 0.3, 1.0));
                }
                self.ui.hearts(g.player.health, x0, y0 - 18.0 * s);
                let points = g.inv.armor_points();
                if points > 0 {
                    self.ui.armor_bar(points, x0, y0 - 29.0 * s);
                }
                self.ui.hunger_bar(g.player.hunger.food, x0 + slot * 9.0, y0 - 18.0 * s);
            }
            if g.held_name > 0.0 {
                let held = g.inv.held();
                if held != AIR {
                    let a = g.held_name.min(1.0);
                    self.ui.text_centered(item_name(held), w / 2.0, y0 - if g.creative { 6.0 } else { 16.0 } * s, 10.0, Color::new(1.0, 1.0, 1.0, a));
                }
            }
        }

        // Chat-ish messages. While typing, the whole log (scrollable) instead.
        let typing = self.chat.is_some();
        let lines: Vec<(&str, f32)> = if typing {
            let n = g.chat_log.len();
            let end = n.saturating_sub(self.chat_scroll);
            g.chat_log.iter().take(end).rev().take(14).map(|m| (m.as_str(), 1.0)).collect()
        } else {
            g.messages.iter().rev().map(|(m, t)| (m.as_str(), t.min(1.0))).collect()
        };
        for (i, (m, a)) in lines.into_iter().enumerate() {
            let y = h - 40.0 * s - i as f32 * 11.0 * s;
            let tw = self.ui.text_width(m, 9.0);
            draw_rectangle(4.0 * s, y - 9.0 * s, tw + 6.0 * s, 11.0 * s, Color::new(0.0, 0.0, 0.0, 0.4 * a));
            self.ui.text(m, 7.0 * s, y, 9.0, Color::new(1.0, 1.0, 1.0, a));
        }
        if typing && self.chat_scroll > 0 {
            self.ui.text(&format!("(scrolled back {} lines: Page Down to return)", self.chat_scroll), 7.0 * s, h - 40.0 * s - 14.5 * 11.0 * s, 8.0, GRAY);
        }

        if let Some(line) = &self.chat {
            let y = h - 30.0 * s;
            draw_rectangle(4.0 * s, y - 10.0 * s, w - 8.0 * s, 13.0 * s, Color::new(0.0, 0.0, 0.0, 0.6));
            let caret = if (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { " " };
            self.ui.text(&format!("> {line}{caret}"), 7.0 * s, y, 9.0, WHITE);
        }

        if self.show_debug {
            let p = g.player.body.pos;
            let (hgt, biome) = g.world.generator.column(p.x.floor() as i32, p.z.floor() as i32);
            let facing = ["north (-Z)", "east (+X)", "south (+Z)", "west (-X)"][((g.player.yaw / std::f32::consts::FRAC_PI_2 + 0.5).floor() as i32).rem_euclid(4) as usize];
            let hours = ((g.time * 24.0 + 6.0) % 24.0) as i32;
            let lines = [
                format!("Minceraft {} ({:.0} fps)", paths::version(), self.fps),
                format!("XYZ: {:.2} / {:.2} / {:.2}", p.x, p.y, p.z),
                format!("Facing: {facing}"),
                format!("Biome: {} (surface {hgt})", biome.name()),
                {
                    // The closest thing the generator built, within a few chunks.
                    let (pcx, pcz) = ((p.x / 16.0).floor() as i32, (p.z / 16.0).floor() as i32);
                    let near = (-6..=6)
                        .flat_map(|dz| (-6..=6).map(move |dx| (pcx + dx, pcz + dz)))
                        .filter_map(|(cx, cz)| g.world.generator.site(cx, cz))
                        .min_by_key(|s| (s.origin.x as f32 - p.x).hypot(s.origin.z as f32 - p.z) as i32);
                    match near {
                        Some(s) => format!("Nearest structure: {} at {}, {}, {}", s.kind.name(), s.origin.x, s.origin.y, s.origin.z),
                        None => "Nearest structure: none nearby".to_string(),
                    }
                },
                format!("Chunks: {} meshed, {} loaded; {} regions of edits in memory", self.renderer.chunks.len(), g.world.chunks.len(), g.world.regions_loaded()),
                {
                    // Palette-packed block storage vs. two bytes per block.
                    let (bytes, bits) = g.world.chunks.values().fold((0, 0), |(b, t), c| (b + c.blocks.bytes(), t + c.blocks.total_bits()));
                    let blocks = (g.world.chunks.len() * (world::CW * world::CW * world::CH) as usize).max(1);
                    format!("Block memory: {:.1} MB ({:.1} bits/block, flat would be {:.1} MB)", bytes as f64 / 1e6, bits as f64 / blocks as f64, blocks as f64 * 2.0 / 1e6)
                },
                {
                    // Light where you stand, and how much of it is stored cell by cell.
                    let e = g.player.eye().floor().as_ivec3();
                    let detailed: usize = g.world.chunks.values().map(|c| c.light.detailed()).sum();
                    format!(
                        "Light: sky {} block {}  ({} of {} sections stored in full)",
                        g.world.sky_level(e.x, e.y, e.z),
                        g.world.block_level(e.x, e.y, e.z),
                        detailed,
                        g.world.chunks.len() * (world::CH / 16) as usize
                    )
                },
                format!("Mobs: {}  Particles: {}", g.mobs.len(), g.particles.len()),
                format!("Time: {:02}:00  Daylight: {:.2}  Day {} ({}){}", hours, g.daylight(), g.day + 1, moon::NAMES[g.moon_phase() as usize], g.season().map(|s| format!(", {}", s.name())).unwrap_or_default()),
                format!("Seed: {}  Mode: {}{}", g.world.seed(), g.mode().name(), if g.rules.hardcore { " (hardcore)" } else { "" }),
                match &g.net {
                    None => "Network: single player".to_string(),
                    Some(multiplayer::Net::Host(srv)) => format!("Network: hosting on port {} ({} players)", srv.port, g.player_count()),
                    Some(multiplayer::Net::Client(_)) => format!("Network: connected as {} ({} players)", g.player_name, g.player_count()),
                },
            ];
            // Minceraft-style: each line on its own dark strip, game on the left...
            let backed = |ui: &Ui, text: &str, x: f32, y: f32, right: bool| {
                let tw = ui.text_width(text, 9.0);
                let x = if right { x - tw } else { x };
                draw_rectangle(x - 2.0 * s, y - 8.5 * s, tw + 4.0 * s, 10.0 * s, Color::new(0.2, 0.2, 0.2, 0.55));
                ui.text(text, x, y, 9.0, WHITE);
            };
            for (i, l) in lines.iter().enumerate() {
                backed(&self.ui, l, 4.0 * s, (12.0 + i as f32 * 10.0) * s, false);
            }
            // ...the machine on the right...
            let ft: Vec<f32> = self.frame_times.iter().copied().collect();
            let avg = ft.iter().sum::<f32>() / ft.len().max(1) as f32;
            let worst = ft.iter().fold(0.0f32, |a, &b| a.max(b));
            let target = g.world.raycast(g.player.eye(), g.player.look_dir(), 8.0).map(|hit| {
                let id = g.world.get_v(hit.pos);
                format!("Looking at: {} ({}, {}, {})", block::item_name(id), hit.pos.x, hit.pos.y, hit.pos.z)
            });
            let right = [
                Some(format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)),
                Some(format!("Window: {}x{}  UI size {:.1}", w as i32, h as i32, s)),
                Some(format!("GPU: {}", self.gpu.0)),
                Some(format!("OpenGL {}", self.gpu.1)),
                Some(format!("Render distance: {} chunks  Clouds: {}", self.settings.render_distance, if self.settings.fancy_clouds { "Fancy" } else { "Fast" })),
                Some(format!("Frame time: {avg:.1} ms average, {worst:.1} ms worst")),
                {
                    // Where the time goes on the CPU; the rest of a frame is the graphics card (or vsync).
                    let [tick, mesh, scene, draw, ui] = self.stage_ms;
                    let cpu = tick + mesh + scene + draw + ui;
                    Some(format!("ms: game {tick:.1} mesh {mesh:.1} scene {scene:.1} draw {draw:.1} UI {ui:.1} | GPU {:.1}", (avg - cpu).max(0.0)))
                },
                target,
            ];
            for (i, l) in right.iter().flatten().enumerate() {
                backed(&self.ui, l, w - 4.0 * s, (12.0 + i as f32 * 10.0) * s, true);
            }
            // ...and a frame-time graph in the corner: green under 60 fps, yellow under 30, red beyond.
            let (gw, gh) = (FRAME_GRAPH as f32 * s * 0.6, 40.0 * s);
            let (gx, gy) = (4.0 * s, h - 60.0 * s);
            draw_rectangle(gx, gy - gh, gw, gh, Color::new(0.0, 0.0, 0.0, 0.45));
            let bar = gw / FRAME_GRAPH as f32;
            let per_ms = gh / 50.0;
            for (i, &ms) in ft.iter().enumerate() {
                let col = if ms <= 17.0 { Color::new(0.3, 0.9, 0.3, 0.9) } else if ms <= 34.0 { Color::new(0.95, 0.85, 0.2, 0.9) } else { Color::new(0.95, 0.3, 0.25, 0.9) };
                let bh = (ms * per_ms).min(gh);
                draw_rectangle(gx + i as f32 * bar, gy - bh, bar.max(1.0), bh, col);
            }
            for (ms, label) in [(16.7, "60 fps"), (33.3, "30 fps")] {
                let ly = gy - ms * per_ms;
                draw_line(gx, ly, gx + gw, ly, 1.0, Color::new(1.0, 1.0, 1.0, 0.5));
                self.ui.text(label, gx + gw + 3.0 * s, ly + 3.0 * s, 7.0, WHITE);
            }
        }
        self.fishing_hud();
        self.brushing_hud();
        self.raid_hud();
        // The Deep Dark's darkness: pulsing in after a shriek, and faintly whenever the Hush is near.
        let hush = self.game.mobs.iter().filter(|m| m.kind == entity::MobKind::Hush).map(|m| m.body.pos.distance(self.game.player.body.pos)).fold(f32::MAX, f32::min);
        let near = (1.0 - hush / 24.0).clamp(0.0, 1.0) * 0.45;
        let dark = (self.game.darkness / 4.0).min(1.0) * 0.8;
        let pulse = 0.85 + (get_time() as f32 * 2.2).sin() * 0.15;
        let a = (dark.max(near) * pulse).min(0.9);
        if a > 0.01 && !self.game.menu {
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.02, a));
        }
        self.toast();
    }

    /// A compass and a map, while you hold them.
    pub(crate) fn navigation_hud(&mut self, dt: f32) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        let held = self.game.inv.held();
        if self.game.spyglassing() {
            // Round glass, dark all around it.
            let r = h * 0.42;
            let (cx, cy) = (w / 2.0, h / 2.0);
            let dark = Color::new(0.0, 0.0, 0.0, 0.92);
            draw_rectangle(0.0, 0.0, cx - r, h, dark);
            draw_rectangle(cx + r, 0.0, w - cx - r, h, dark);
            for k in 0..48 {
                let y0 = cy - r + k as f32 * r * 2.0 / 48.0;
                let dy = (y0 + r / 48.0 - cy) / r;
                let half = r * (1.0 - dy * dy).max(0.0).sqrt();
                draw_rectangle(cx - r, y0, r - half, r * 2.0 / 48.0 + 1.0, dark);
                draw_rectangle(cx + half, y0, r - half, r * 2.0 / 48.0 + 1.0, dark);
            }
            draw_rectangle(cx - r, 0.0, 2.0 * r, cy - r, dark);
            draw_rectangle(cx - r, cy + r, 2.0 * r, h - cy - r, dark);
            draw_circle_lines(cx, cy, r, 4.0 * s, Color::new(0.55, 0.32, 0.2, 1.0));
            return;
        }
        if held == block::COMPASS {
            let r = 22.0 * s;
            let (cx, cy) = (w / 2.0, 34.0 * s);
            draw_circle(cx, cy, r + 2.0 * s, Color::new(0.2, 0.2, 0.22, 0.9));
            draw_circle(cx, cy, r, Color::new(0.92, 0.9, 0.84, 0.95));
            let g = &self.game;
            // Home, or a Lodestone (spinning if it's been broken; see gadgets.rs).
            let target = g.compass_target();
            let a = match target {
                Some(t) => navigation::compass_needle(g.player.body.pos, g.player.yaw, t, g.clock),
                None => g.clock * 7.0,
            };
            let (dx, dy) = (a.sin(), -a.cos());
            draw_line(cx, cy, cx + dx * r * 0.85, cy + dy * r * 0.85, 3.0 * s, Color::new(0.85, 0.1, 0.1, 1.0));
            draw_line(cx, cy, cx - dx * r * 0.5, cy - dy * r * 0.5, 3.0 * s, Color::new(0.3, 0.3, 0.35, 1.0));
            if !scorch::in_scorch(g.player.body.pos.x) {
                let what = if g.lodestone.is_some() { "Lodestone" } else { "Home" };
                let text = match target {
                    Some(t) => format!("{what}: {} blocks", Vec2::new(t.x - g.player.body.pos.x, t.z - g.player.body.pos.z).length() as i32),
                    None => "Lodestone: gone".to_string(),
                };
                self.ui.text_centered(&text, cx, cy + r + 12.0 * s, 8.0, WHITE);
            }
        }
        if held == block::MAP {
            self.map_timer -= dt;
            if self.map_timer <= 0.0 || self.map_tex.is_none() {
                self.map_timer = 0.5;
                let scale = navigation::ZOOMS[self.game.map_zoom as usize % navigation::ZOOMS.len()];
                let px = navigation::map_pixels(&self.game.world, self.game.player.body.pos, &self.map_colors, scale);
                let n = navigation::MAP_SIZE as u16;
                match &self.map_tex {
                    Some(t) => t.update_from_bytes(n as u32, n as u32, &px),
                    None => {
                        let t = Texture2D::from_rgba8(n, n, &px);
                        t.set_filter(FilterMode::Nearest);
                        self.map_tex = Some(t);
                    }
                }
            }
            if let Some(t) = &self.map_tex {
                let size = 150.0 * s;
                let (x, y) = (w - size - 10.0 * s, 10.0 * s);
                draw_rectangle(x - 4.0 * s, y - 4.0 * s, size + 8.0 * s, size + 8.0 * s, Color::new(0.55, 0.43, 0.26, 1.0));
                draw_texture_ex(t, x, y, WHITE, DrawTextureParams { dest_size: Some(vec2(size, size)), ..Default::default() });
                // You are here (pointing the way you face).
                let (cx, cy) = (x + size / 2.0, y + size / 2.0);
                let yaw = self.game.player.yaw;
                let (fx, fy) = (yaw.sin(), -yaw.cos());
                let (rx, ry) = (-fy, fx);
                // Banners that are up show on the map, in their colours.
                let scale = navigation::ZOOMS[self.game.map_zoom as usize % navigation::ZOOMS.len()] as f32;
                let per_px = size / navigation::MAP_SIZE as f32;
                let me = self.game.player.body.pos;
                for (p, &(design, _)) in &self.game.banners {
                    let (dx, dz) = ((p.x as f32 + 0.5 - me.x) / scale, (p.z as f32 + 0.5 - me.z) / scale);
                    let half = navigation::MAP_SIZE as f32 / 2.0;
                    if dx.abs() < half - 1.0 && dz.abs() < half - 1.0 {
                        let d = banners::Design::from_bits(design);
                        let c = carpentry::colour_rgb(d.base as usize);
                        let (bx, by) = (cx + dx * per_px, cy + dz * per_px);
                        draw_rectangle(bx - 3.0 * s, by - 5.0 * s, 6.0 * s, 7.0 * s, Color::new(0.1, 0.08, 0.05, 1.0));
                        draw_rectangle(bx - 2.0 * s, by - 4.0 * s, 4.0 * s, 5.0 * s, Color::from_rgba(c[0], c[1], c[2], 255));
                    }
                }
                // Where you last died: a dark cross.
                if let Some(d) = self.game.last_death.filter(|_| !scorch::in_scorch(me.x)) {
                    let (dx, dz) = ((d.x - me.x) / scale, (d.z - me.z) / scale);
                    let half = navigation::MAP_SIZE as f32 / 2.0;
                    if dx.abs() < half - 1.0 && dz.abs() < half - 1.0 {
                        let (bx, by) = (cx + dx * per_px, cy + dz * per_px);
                        let k = 4.0 * s;
                        let ink = Color::new(0.15, 0.05, 0.05, 1.0);
                        draw_line(bx - k, by - k, bx + k, by + k, 2.5 * s, ink);
                        draw_line(bx - k, by + k, bx + k, by - k, 2.5 * s, ink);
                    }
                }
                self.ui.text(&format!("1:{}", scale as i32), x + 4.0 * s, y + size - 4.0 * s, 8.0, Color::new(0.2, 0.15, 0.1, 1.0));
                let k = 6.0 * s;
                draw_triangle(vec2(cx + fx * k, cy + fy * k), vec2(cx - fx * k * 0.6 + rx * k * 0.6, cy - fy * k * 0.6 + ry * k * 0.6), vec2(cx - fx * k * 0.6 - rx * k * 0.6, cy - fy * k * 0.6 - ry * k * 0.6), Color::new(0.9, 0.1, 0.1, 1.0));
            }
        }
        if held == block::TREASURE_MAP {
            self.treasure_map_hud(dt);
        }
    }

    /// A Treasure Map: the land round the X (as far as anyone's seen it), you
    /// on it once you're close, and how far off it is (see treasure.rs).
    fn treasure_map_hud(&mut self, dt: f32) {
        let (w, s) = (screen_width(), self.ui.s);
        let wear = self.game.inv.wear[self.game.inv.selected];
        let me = self.game.player.body.pos;
        let size = 150.0 * s;
        let (x, y) = (w - size - 10.0 * s, 10.0 * s);
        draw_rectangle(x - 4.0 * s, y - 4.0 * s, size + 8.0 * s, size + 8.0 * s, Color::new(0.55, 0.43, 0.26, 1.0));
        let caption = treasure::caption(wear, me);
        if let Some((tx, tz)) = treasure::marked(wear) {
            const SCALE: i32 = 2;
            self.map_timer -= dt;
            if self.map_timer <= 0.0 || self.map_tex.is_none() {
                self.map_timer = 0.5;
                let px = navigation::map_pixels(&self.game.world, vec3(tx as f32 + 0.5, 64.0, tz as f32 + 0.5), &self.map_colors, SCALE);
                let n = navigation::MAP_SIZE as u16;
                match &self.map_tex {
                    Some(t) => t.update_from_bytes(n as u32, n as u32, &px),
                    None => {
                        let t = Texture2D::from_rgba8(n, n, &px);
                        t.set_filter(FilterMode::Nearest);
                        self.map_tex = Some(t);
                    }
                }
            }
            if let Some(t) = &self.map_tex {
                draw_texture_ex(t, x, y, Color::new(1.0, 0.92, 0.8, 1.0), DrawTextureParams { dest_size: Some(vec2(size, size)), ..Default::default() });
            }
            let (cx, cy) = (x + size / 2.0, y + size / 2.0);
            let red = Color::new(0.8, 0.1, 0.1, 1.0);
            let k = 6.0 * s;
            draw_line(cx - k, cy - k, cx + k, cy + k, 3.0 * s, red);
            draw_line(cx - k, cy + k, cx + k, cy - k, 3.0 * s, red);
            // You, once you're on the map.
            let per_px = size / navigation::MAP_SIZE as f32 / SCALE as f32;
            let (dx, dz) = ((me.x - tx as f32 - 0.5) * per_px, (me.z - tz as f32 - 0.5) * per_px);
            if dx.abs() < size / 2.0 - 3.0 * s && dz.abs() < size / 2.0 - 3.0 * s {
                draw_circle(cx + dx, cy + dz, 3.5 * s, Color::new(1.0, 1.0, 1.0, 1.0));
                draw_circle_lines(cx + dx, cy + dz, 3.5 * s, s, Color::new(0.1, 0.1, 0.1, 1.0));
            }
        } else {
            draw_rectangle(x, y, size, size, Color::new(0.77, 0.7, 0.55, 1.0));
        }
        self.ui.text_centered(&caption, x + size / 2.0, y + size + 14.0 * s, 8.0, WHITE);
    }

    /// A boss's health, across the top while it's near: the Hollow Wyrm, or
    /// the nearest mod-defined boss.
    pub(crate) fn boss_bar(&self) {
        let g = &self.game;
        let me = g.player.body.pos;
        let wyrm = g.mobs.iter().find(|m| m.kind == entity::MobKind::Wyrm && m.body.pos.distance(me) < 150.0);
        let modded = || {
            g.mobs
                .iter()
                .filter(|m| m.health > 0.0 && m.kind.mod_def().is_some_and(|d| d.boss) && m.body.pos.distance(me) < 64.0)
                .min_by(|a, b| a.body.pos.distance(me).total_cmp(&b.body.pos.distance(me)))
        };
        let Some(m) = wyrm.or_else(modded) else { return };
        let (name, fill, back, label) = match m.kind.mod_def() {
            Some(d) => (d.name.as_str(), Color::new(0.85, 0.2, 0.2, 1.0), Color::new(0.15, 0.04, 0.04, 0.9), Color::new(1.0, 0.7, 0.6, 1.0)),
            None => ("Hollow Wyrm", Color::new(0.75, 0.25, 0.9, 1.0), Color::new(0.1, 0.05, 0.12, 0.9), Color::new(0.85, 0.6, 1.0, 1.0)),
        };
        let (w, s) = (screen_width(), self.ui.s);
        let bw = (180.0 * s).min(w * 0.7);
        let (x, y) = (w / 2.0 - bw / 2.0, 18.0 * s);
        let frac = (m.health / m.kind.max_health()).clamp(0.0, 1.0);
        self.ui.text_centered(name, w / 2.0, y - 3.0 * s, 9.0, label);
        draw_rectangle(x, y, bw, 5.0 * s, back);
        draw_rectangle(x, y, bw * frac, 5.0 * s, fill);
    }

    pub(crate) fn name_tags(&self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        let eye = self.game.player.eye();
        for p in self.game.peers.values().filter(|p| p.alive()) {
            let at = p.pos + Vec3::Y * 2.15;
            if at.distance(eye) > 64.0 {
                continue;
            }
            let clip = self.last_view_proj * at.extend(1.0);
            if clip.w < 0.1 {
                continue;
            }
            let (sx, sy) = ((clip.x / clip.w * 0.5 + 0.5) * w, (0.5 - clip.y / clip.w * 0.5) * h);
            let tw = self.ui.text_width(&p.name, 9.0);
            draw_rectangle(sx - tw / 2.0 - 3.0 * s, sy - 10.0 * s, tw + 6.0 * s, 12.0 * s, Color::new(0.0, 0.0, 0.0, 0.45));
            self.ui.text_centered(&p.name, sx, sy, 9.0, WHITE);
        }
        // Mobs with Name Tags.
        for m in &self.game.mobs {
            let Some(name) = self.game.mob_names.get(&m.id) else { continue };
            let at = m.body.pos + Vec3::Y * (m.body.height + 0.4);
            if at.distance(eye) > 32.0 {
                continue;
            }
            let clip = self.last_view_proj * at.extend(1.0);
            if clip.w < 0.1 {
                continue;
            }
            let (sx, sy) = ((clip.x / clip.w * 0.5 + 0.5) * w, (0.5 - clip.y / clip.w * 0.5) * h);
            let tw = self.ui.text_width(name, 8.0);
            draw_rectangle(sx - tw / 2.0 - 3.0 * s, sy - 9.0 * s, tw + 6.0 * s, 11.0 * s, Color::new(0.0, 0.0, 0.0, 0.4));
            self.ui.text_centered(name, sx, sy, 8.0, Color::new(1.0, 0.95, 0.8, 1.0));
        }
    }
}
