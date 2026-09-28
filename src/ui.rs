//! Immediate-mode 2D UI drawn with macroquad on top of the 3D world.

use crate::block::*;
use crate::texture::*;
use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;

pub struct Ui {
    pub tex: Texture2D,
    /// GUI scale: 1 "pixel" of UI art in screen pixels.
    pub s: f32,
    pub clicked: bool,
    pub rclicked: bool,
    /// Set whenever a button fires this frame, so the app can play a click.
    pub pressed: std::cell::Cell<bool>,
}

pub const PANEL: Color = Color::new(0.08, 0.08, 0.1, 0.88);
pub const SLOT_BG: Color = Color::new(0.25, 0.25, 0.28, 0.9);

impl Ui {
    pub fn new(tex: Texture2D) -> Self {
        Ui { tex, s: 2.0, clicked: false, rclicked: false, pressed: std::cell::Cell::new(false) }
    }

    pub fn begin_frame(&mut self) {
        let h = screen_height();
        self.s = (h / 300.0).clamp(1.5, 4.0);
        self.clicked = is_mouse_button_pressed(MouseButton::Left);
        self.rclicked = is_mouse_button_pressed(MouseButton::Right);
    }

    pub fn font(&self, px: f32) -> u16 {
        (px * self.s).round() as u16
    }

    pub fn text(&self, t: &str, x: f32, y: f32, px: f32, color: Color) {
        let size = self.font(px);
        let off = (self.s * 0.6).max(1.0);
        draw_text_ex(t, x + off, y + off, TextParams { font_size: size, color: Color::new(0.0, 0.0, 0.0, color.a * 0.7), ..Default::default() });
        draw_text_ex(t, x, y, TextParams { font_size: size, color, ..Default::default() });
    }

    pub fn text_width(&self, t: &str, px: f32) -> f32 {
        measure_text(t, None, self.font(px), 1.0).width
    }

    /// Shorten text with "..." so it fits in `max` pixels.
    pub fn fit(&self, t: &str, px: f32, max: f32) -> String {
        if self.text_width(t, px) <= max {
            return t.to_string();
        }
        let mut s: String = t.to_string();
        while !s.is_empty() && self.text_width(&format!("{s}..."), px) > max {
            s.pop();
        }
        format!("{s}...")
    }

    pub fn text_centered(&self, t: &str, cx: f32, y: f32, px: f32, color: Color) {
        let w = self.text_width(t, px);
        self.text(t, cx - w / 2.0, y, px, color);
    }

    pub fn hovered(&self, r: Rect) -> bool {
        let (mx, my) = mouse_position();
        r.contains(vec2(mx, my))
    }

    pub fn button(&self, r: Rect, label: &str, enabled: bool) -> bool {
        let hov = enabled && self.hovered(r);
        let bg = if !enabled {
            Color::new(0.2, 0.2, 0.2, 0.8)
        } else if hov {
            Color::new(0.45, 0.5, 0.75, 0.95)
        } else {
            Color::new(0.35, 0.35, 0.38, 0.92)
        };
        draw_rectangle(r.x, r.y, r.w, r.h, bg);
        let b = self.s.max(1.0);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, b * 1.5, if hov { WHITE } else { Color::new(0.0, 0.0, 0.0, 0.8) });
        draw_rectangle(r.x + b, r.y + b, r.w - 2.0 * b, b, Color::new(1.0, 1.0, 1.0, 0.15));
        let col = if enabled { if hov { Color::new(1.0, 1.0, 0.6, 1.0) } else { WHITE } } else { GRAY };
        let size = 10.0;
        let dims = measure_text(label, None, self.font(size), 1.0);
        self.text(label, r.x + (r.w - dims.width) / 2.0, r.y + r.h / 2.0 + dims.offset_y / 2.0 - b, size, col);
        let fired = hov && self.clicked;
        if fired {
            self.pressed.set(true);
        }
        fired
    }

    fn tile_src(tile: u16) -> Rect {
        let t = TILE as f32;
        Rect::new((tile % TILES_PER_ROW) as f32 * t, (tile / TILES_PER_ROW) as f32 * t, t, t)
    }

    pub fn tile(&self, tile: u16, x: f32, y: f32, size: f32, color: Color) {
        draw_texture_ex(&self.tex, x, y, color, DrawTextureParams { dest_size: Some(vec2(size, size)), source: Some(Self::tile_src(tile)), ..Default::default() });
    }

    /// Item icon: blocks become a little isometric cube, everything else a flat sprite.
    pub fn icon(&self, item: u8, x: f32, y: f32, size: f32) {
        if is_block_item(item) && block(item).model == Model::Cube {
            let t = block(item).tex;
            let cx = x + size / 2.0;
            let p = |fx: f32, fy: f32| vec2(x + size * fx, y + size * fy);
            let top = [p(0.5, 0.04), p(0.95, 0.27), p(0.5, 0.5), p(0.05, 0.27)];
            let left = [p(0.05, 0.27), p(0.5, 0.5), p(0.5, 0.96), p(0.05, 0.73)];
            let right = [p(0.5, 0.5), p(0.95, 0.27), p(0.95, 0.73), p(0.5, 0.96)];
            let _ = cx;
            let mut verts = Vec::with_capacity(12);
            let mut idx: Vec<u16> = Vec::with_capacity(18);
            for (quad, tile, shade, uvs) in [
                (top, t[0], 1.0, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]),
                (left, t[1], 0.78, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]),
                (right, t[1], 0.6, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]),
            ] {
                let (u0, v0, s) = tile_uv(tile);
                let b = verts.len() as u16;
                for (i, q) in quad.iter().enumerate() {
                    let c = Color::new(shade, shade, shade, 1.0);
                    verts.push(Vertex::new(q.x, q.y, 0.0, u0 + uvs[i][0] * s, v0 + uvs[i][1] * s, c));
                }
                idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
            }
            draw_mesh(&Mesh { vertices: verts, indices: idx, texture: Some(self.tex.clone()) });
        } else {
            let tile = if is_block_item(item) { block(item).tex[1] } else { item_tile(item) };
            self.tile(tile, x, y, size, WHITE);
        }
    }

    pub fn stack(&self, stack: Option<(u8, u8)>, x: f32, y: f32, size: f32, show_count: bool) {
        if let Some((id, n)) = stack {
            let pad = size * 0.12;
            self.icon(id, x + pad, y + pad, size - pad * 2.0);
            if n > 1 && show_count {
                let t = n.to_string();
                let w = self.text_width(&t, 8.0);
                self.text(&t, x + size - w - self.s, y + size - self.s * 1.5, 8.0, WHITE);
            }
        }
    }

    /// Returns (clicked, right-clicked, hovered).
    pub fn slot(&self, stack: Option<(u8, u8)>, x: f32, y: f32, size: f32, selected: bool) -> (bool, bool, bool) {
        let r = Rect::new(x, y, size, size);
        let hov = self.hovered(r);
        draw_rectangle(x, y, size, size, if hov { Color::new(0.45, 0.45, 0.5, 0.95) } else { SLOT_BG });
        draw_rectangle_lines(x, y, size, size, self.s, Color::new(0.0, 0.0, 0.0, 0.7));
        if selected {
            draw_rectangle_lines(x - self.s, y - self.s, size + 2.0 * self.s, size + 2.0 * self.s, self.s * 1.5, WHITE);
        }
        self.stack(stack, x, y, size, true);
        (hov && self.clicked, hov && self.rclicked, hov)
    }

    /// A single-line text box; returns true when clicked (to take focus).
    pub fn text_field(&self, r: Rect, text: &str, focused: bool) -> bool {
        let hov = self.hovered(r);
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.05, 0.05, 0.06, 0.95));
        let border = if focused { WHITE } else if hov { LIGHTGRAY } else { GRAY };
        draw_rectangle_lines(r.x, r.y, r.w, r.h, self.s * 1.5, border);
        let caret = if focused && (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { "" };
        self.text(&format!("{text}{caret}"), r.x + 5.0 * self.s, r.y + r.h * 0.68, 10.0, WHITE);
        hov && self.clicked
    }

    pub fn tooltip(&self, text: &str) {
        let (mx, my) = mouse_position();
        let w = self.text_width(text, 9.0) + 8.0 * self.s;
        let h = 14.0 * self.s;
        let x = (mx + 10.0 * self.s).min(screen_width() - w);
        draw_rectangle(x, my - h, w, h, Color::new(0.1, 0.0, 0.2, 0.95));
        draw_rectangle_lines(x, my - h, w, h, self.s, Color::new(0.4, 0.1, 0.9, 1.0));
        self.text(text, x + 4.0 * self.s, my - 4.0 * self.s, 9.0, WHITE);
    }

    pub fn hearts(&self, health: f32, x: f32, y: f32) {
        let size = 9.0 * self.s;
        for i in 0..10 {
            let hx = x + i as f32 * (size - self.s);
            let v = health - i as f32 * 2.0;
            let tile = if v >= 2.0 {
                T_HEART
            } else if v >= 1.0 {
                T_HALF_HEART
            } else {
                T_HEART_EMPTY
            };
            self.tile(tile, hx, y, size, WHITE);
        }
    }

    pub fn crosshair(&self) {
        let (cx, cy) = (screen_width() / 2.0, screen_height() / 2.0);
        let (l, t) = (6.0 * self.s, self.s.max(1.5));
        let dark = Color::new(0.0, 0.0, 0.0, 0.5);
        draw_rectangle(cx - l - 1.0, cy - t / 2.0 - 1.0, l * 2.0 + 2.0, t + 2.0, dark);
        draw_rectangle(cx - t / 2.0 - 1.0, cy - l - 1.0, t + 2.0, l * 2.0 + 2.0, dark);
        draw_rectangle(cx - l, cy - t / 2.0, l * 2.0, t, WHITE);
        draw_rectangle(cx - t / 2.0, cy - l, t, l * 2.0, WHITE);
    }

    /// The game's logo, built out of cobblestun pixels.
    pub fn logo(&self, cx: f32, y: f32, px: f32) {
        const GLYPHS: [(char, [&str; 5]); 8] = [
            ('M', ["#...#", "##.##", "#.#.#", "#...#", "#...#"]),
            ('I', ["###", ".#.", ".#.", ".#.", "###"]),
            ('N', ["#...#", "##..#", "#.#.#", "#..##", "#...#"]),
            ('C', [".###", "#...", "#...", "#...", ".###"]),
            ('E', ["####", "#...", "###.", "#...", "####"]),
            ('R', ["###.", "#..#", "###.", "#.#.", "#..#"]),
            ('A', [".##.", "#..#", "####", "#..#", "#..#"]),
            ('F', ["####", "#...", "###.", "#...", "#..."]),
        ];
        let word = "MINCERAFT";
        let glyph = |c: char| GLYPHS.iter().find(|g| g.0 == c).map(|g| g.1).unwrap_or(GLYPHS[0].1);
        let t_glyph = ["###", ".#.", ".#.", ".#.", ".#."];
        let width: f32 = word.chars().map(|c| if c == 'T' { 3.0 } else { glyph(c)[0].len() as f32 } + 1.0).sum::<f32>() - 1.0;
        let mut x = cx - width * px / 2.0;
        for (ci, c) in word.chars().enumerate() {
            let rows: [&str; 5] = if c == 'T' { t_glyph } else { glyph(c) };
            let wobble = ((get_time() * 2.0 + ci as f64 * 0.5).sin() * px as f64 * 0.08) as f32;
            for (ry, row) in rows.iter().enumerate() {
                for (rx, ch) in row.chars().enumerate() {
                    if ch == '#' {
                        let (bx, by) = (x + rx as f32 * px, y + ry as f32 * px + wobble);
                        draw_rectangle(bx + px * 0.12, by + px * 0.12, px, px, Color::new(0.0, 0.0, 0.0, 0.6));
                        self.tile(T_COBBLE, bx, by, px, Color::new(0.95, 0.95, 0.95, 1.0));
                        draw_rectangle_lines(bx, by, px, px, 1.0, Color::new(0.0, 0.0, 0.0, 0.35));
                    }
                }
            }
            x += (rows[0].len() as f32 + 1.0) * px;
        }
    }
}
