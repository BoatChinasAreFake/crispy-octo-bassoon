//! Creative building tools (v0.8): a selection, `/fill`, `/clone`, the
//! **Selection Wand**, saved builds and the **Structure Block**.
//!
//! - Everyone has a selection: two corners, set with `/pos1` and `/pos2`
//!   (where you stand, or at x y z), or with the Selection Wand in creative
//!   (hit a block for the first corner, use it on one for the second).
//! - `/fill <block>` fills the selection (or two corners given in full), with
//!   `hollow`, `outline`, `keep` or `replace <block>` to be choosier.
//! - `/clone <x y z>` copies the selection so its low corner lands there.
//! - `/structure save|load|export|list|delete <name>` keeps builds with the
//!   world, pastes them, and writes one out as a mod's `[structure]` template.
//! - A **Structure Block** holds one build: use it with the wand to save your
//!   selection into it, and with an empty hand to paste its build on top.
//!
//! All of it runs where the world lives (the host or the server), so joined
//! players use it through commands; edits reach everyone as usual.

use crate::block::*;
use crate::game::Game;
use macroquad::math::{ivec3, IVec3};
use std::collections::BTreeMap;

/// The most blocks one command may touch.
pub const MAX_VOLUME: i64 = 128 * 1024;

/// A copied build: its size and its blocks, x fastest, then z, then y.
#[derive(Clone, Debug, PartialEq)]
pub struct Build {
    pub size: IVec3,
    pub blocks: Vec<Id>,
}

impl Build {
    fn at(&self, x: i32, y: i32, z: i32) -> Id {
        self.blocks[((y * self.size.z + z) * self.size.x + x) as usize]
    }

    /// How many blocks aren't air.
    pub fn solid(&self) -> usize {
        self.blocks.iter().filter(|&&b| b != AIR).count()
    }
}

/// The low and high corners of the box two corners make.
pub fn corners(a: IVec3, b: IVec3) -> (IVec3, IVec3) {
    (a.min(b), a.max(b))
}

/// How many blocks the box between two corners holds.
pub fn volume(a: IVec3, b: IVec3) -> i64 {
    let (lo, hi) = corners(a, b);
    let s = hi - lo + IVec3::ONE;
    s.x as i64 * s.y as i64 * s.z as i64
}

/// Copy the box between `a` and `b` from `get`.
pub fn capture(get: impl Fn(IVec3) -> Id, a: IVec3, b: IVec3) -> Build {
    let (lo, hi) = corners(a, b);
    let size = hi - lo + IVec3::ONE;
    let mut blocks = Vec::with_capacity((size.x * size.y * size.z) as usize);
    for y in 0..size.y {
        for z in 0..size.z {
            for x in 0..size.x {
                blocks.push(get(lo + ivec3(x, y, z)));
            }
        }
    }
    Build { size, blocks }
}

/// Write a build out as a mod's `[structure]` section (see MODDING.md).
pub fn to_template(name: &str, b: &Build) -> Result<String, String> {
    const LETTERS: &str = "#ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789@$%&*+=?!<>^~";
    let mut legend: BTreeMap<Id, char> = BTreeMap::new();
    let mut pool = LETTERS.chars();
    for &id in &b.blocks {
        if id != AIR && !legend.contains_key(&id) {
            let Some(c) = pool.next() else { return Err(format!("too many kinds of block for a template ({} at most)", LETTERS.len())) };
            legend.insert(id, c);
        }
    }
    let mut out = format!("[structure {name}]\nname = {name}\n");
    for (id, c) in &legend {
        out += &format!("block {c} = {}\n", reg().key_of(*id));
    }
    for y in 0..b.size.y {
        out += "layer\n";
        for z in 0..b.size.z {
            let row: String = (0..b.size.x).map(|x| legend.get(&b.at(x, y, z)).copied().unwrap_or('.')).collect();
            out += &row;
            out.push('\n');
        }
    }
    Ok(out)
}

/// Read a build back from a `[structure]` section (as `to_template` writes them).
pub fn from_template(text: &str) -> Result<Build, String> {
    let mut legend: BTreeMap<char, Id> = BTreeMap::from([('.', AIR), ('_', AIR)]);
    let mut layers: Vec<Vec<Vec<char>>> = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if line.starts_with('[') || line.starts_with("name ") || line.starts_with("name=") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("block ") {
            let (k, v) = rest.split_once('=').ok_or_else(|| format!("bad legend line: {line}"))?;
            let c = k.trim().chars().next().ok_or_else(|| format!("bad legend line: {line}"))?;
            let id = reg().lookup(v.trim()).filter(|&i| i < FIRST_ITEM).ok_or_else(|| format!("no block called {}", v.trim()))?;
            legend.insert(c, id);
        } else if line.eq_ignore_ascii_case("layer") {
            layers.push(Vec::new());
        } else if let Some(l) = layers.last_mut() {
            l.push(line.chars().collect());
        }
    }
    let h = layers.len() as i32;
    let d = layers.iter().map(|l| l.len()).max().unwrap_or(0) as i32;
    let w = layers.iter().flatten().map(|r| r.len()).max().unwrap_or(0) as i32;
    if h == 0 || d == 0 || w == 0 {
        return Err("no layers".into());
    }
    let mut blocks = vec![AIR; (w * h * d) as usize];
    for (y, layer) in layers.iter().enumerate() {
        for (z, row) in layer.iter().enumerate() {
            for (x, c) in row.iter().enumerate() {
                let id = *legend.get(c).ok_or_else(|| format!("'{c}' isn't in the legend"))?;
                blocks[((y as i32 * d + z as i32) * w + x as i32) as usize] = id;
            }
        }
    }
    Ok(Build { size: ivec3(w, h, d), blocks })
}

/// Saved builds, packed for the save file: name, then template, each ending in a zero.
pub fn encode(builds: &BTreeMap<String, String>) -> Vec<u8> {
    let mut v = Vec::new();
    for (k, t) in builds {
        v.extend_from_slice(k.as_bytes());
        v.push(0);
        v.extend_from_slice(t.as_bytes());
        v.push(0);
    }
    v
}

pub fn decode(b: &[u8]) -> BTreeMap<String, String> {
    let parts: Vec<String> = b.split(|&c| c == 0).map(|p| String::from_utf8_lossy(p).into_owned()).collect();
    parts.chunks(2).filter(|c| c.len() == 2 && !c[0].is_empty()).map(|c| (c[0].clone(), c[1].clone())).collect()
}

/// A build's name, cleaned: letters, digits, `_` and `-`, at most 32 of them.
pub fn clean_name(name: &str) -> Option<String> {
    let n: String = name.trim().to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').take(32).collect();
    (!n.is_empty()).then_some(n)
}

/// What a Structure Block at `p` keeps its build as.
pub fn block_build_name(p: IVec3) -> String {
    format!("block_{}_{}_{}", p.x, p.y, p.z)
}

/// Where mirror building copies `p`: across the x plane, the z plane, or both
/// (mask 1, 2, 3), about the middle of block `c`.
pub fn mirrors(mask: u8, c: IVec3, p: IVec3) -> Vec<IVec3> {
    let mx = ivec3(2 * c.x - p.x, p.y, p.z);
    let mz = ivec3(p.x, p.y, 2 * c.z - p.z);
    let both = ivec3(2 * c.x - p.x, p.y, 2 * c.z - p.z);
    let mut v = match mask & 3 {
        1 => vec![mx],
        2 => vec![mz],
        3 => vec![mx, mz, both],
        _ => vec![],
    };
    v.retain(|&q| q != p);
    v.dedup();
    v
}

/// A block as it looks in a mirror: stairs turn to face the other way.
pub fn mirrored(id: Id, across_x: bool) -> Id {
    match stairs_of(id) {
        Some((base, f)) => base + match (across_x, f) {
            (true, 1) => 3,
            (true, 3) => 1,
            (false, 0) => 2,
            (false, 2) => 0,
            _ => f,
        } as Id,
        None => id,
    }
}

impl Game {
    /// `/symmetry [x | z | both | off]`: mirror what you build and break
    /// (in creative), about the block you're standing on. Just for you.
    pub fn symmetry_command(&mut self, text: &str) -> Option<Vec<String>> {
        let rest = text.strip_prefix("/symmetry")?;
        if !rest.is_empty() && !rest.starts_with(' ') {
            return None;
        }
        if !self.creative {
            return Some(vec!["Mirror building is for creative mode.".into()]);
        }
        let here = self.player.body.pos.floor().as_ivec3();
        let mask = match rest.trim().to_ascii_lowercase().as_str() {
            "" if self.symmetry.is_some() => 0,
            "" | "both" | "xz" => 3,
            "x" => 1,
            "z" => 2,
            "off" => 0,
            _ => return Some(vec!["Usage: /symmetry [x | z | both | off]".into()]),
        };
        if mask == 0 {
            self.symmetry = None;
            return Some(vec!["Mirror building off.".into()]);
        }
        self.symmetry = Some((mask, here));
        let what = ["", "across x", "across z", "across x and z"][mask as usize];
        Some(vec![format!("Mirror building {what}, about {}, {}, {}.", here.x, here.y, here.z)])
    }

    /// Where mirror building copies `p` to (none when it's off, or out of creative).
    pub fn mirror_spots(&self, p: IVec3) -> Vec<IVec3> {
        match self.symmetry {
            Some((mask, c)) if self.creative => mirrors(mask, c, p),
            _ => Vec::new(),
        }
    }

    /// A block was placed at `p`: put its mirror images down too (where there's room).
    pub fn place_mirrored(&mut self, p: IVec3, id: Id) {
        let Some((mask, c)) = self.symmetry.filter(|_| self.creative) else { return };
        for q in mirrors(mask, c, p) {
            if (1..crate::world::CH).contains(&q.y) && replaceable(self.world.get_v(q)) {
                let across_x = q.x != p.x;
                let across_z = q.z != p.z;
                let mut m = id;
                if across_x {
                    m = mirrored(m, true);
                }
                if across_z {
                    m = mirrored(m, false);
                }
                self.world.set_v(q, m);
            }
        }
        self.advance("mirror_mirror");
    }

    /// Where the ghost preview goes: the spot the held block would take, and
    /// its mirror images (creative, with the option on, looking at a block).
    pub fn ghost_spots(&self) -> Option<Vec<IVec3>> {
        let held = self.inv.held();
        if !self.creative || !self.ghost_preview || held == AIR || held >= FIRST_ITEM || held == SELECTION_WAND {
            return None;
        }
        let Some(crate::game::Target::Block(h)) = &self.target else { return None };
        let hit = self.world.get_v(h.pos);
        let place = if replaceable(hit) { h.pos } else { h.pos + h.normal };
        if !replaceable(self.world.get_v(place)) {
            return None;
        }
        let mut v = vec![place];
        v.extend(self.mirror_spots(place).into_iter().filter(|&q| replaceable(self.world.get_v(q))));
        Some(v)
    }

    /// Put `b` down with its low corner at `at` (air in it clears what's there).
    pub fn paste_build(&mut self, b: &Build, at: IVec3) -> usize {
        let mut n = 0;
        for y in 0..b.size.y {
            for z in 0..b.size.z {
                for x in 0..b.size.x {
                    let p = at + ivec3(x, y, z);
                    if (1..crate::world::CH).contains(&p.y) && self.world.get_v(p) != b.at(x, y, z) {
                        self.world.set_v(p, b.at(x, y, z));
                        n += 1;
                    }
                }
            }
        }
        n
    }

    /// Someone's selection corners.
    pub fn selection(&self, who: &str) -> [Option<IVec3>; 2] {
        self.selections.get(&who.to_ascii_lowercase()).copied().unwrap_or([None, None])
    }

    /// Set corner `k` (0 or 1) of someone's selection.
    pub fn set_corner(&mut self, who: &str, k: usize, p: IVec3) -> String {
        let sel = self.selections.entry(who.to_ascii_lowercase()).or_insert([None, None]);
        sel[k] = Some(p);
        let sel = *sel;
        let size = match sel {
            [Some(a), Some(b)] => format!(" ({} blocks)", volume(a, b)),
            _ => String::new(),
        };
        if self.is_local_player(who) {
            self.my_selection = sel;
        }
        format!("Corner {} set to {}, {}, {}{size}.", k + 1, p.x, p.y, p.z)
    }

    /// The wand was used on `p`: the first corner (hit) or the second (used).
    /// Joined players ask the host (and keep their own copy, to see it).
    pub fn wand_corner(&mut self, k: usize, p: IVec3) {
        self.my_selection[k] = Some(p);
        self.player.swing = 1.0;
        if self.is_client() {
            self.send_chat(&format!("/pos{} {} {} {}", k + 1, p.x, p.y, p.z));
        } else {
            let me = self.player_name.clone();
            let line = self.set_corner(&me, k, p);
            self.msg(line);
        }
    }

    /// The Structure Block at `p` was used (in creative): with the wand,
    /// save the selection into it; empty-handed, paste its build on top.
    pub fn use_structure_block(&mut self, p: IVec3) -> bool {
        if !self.creative {
            return false;
        }
        let held = self.inv.held();
        if self.is_client() {
            if held == SELECTION_WAND {
                self.send_chat(&format!("/structure save {} {} {} {}", block_build_name(p), p.x, p.y, p.z));
            } else if held == AIR {
                self.send_chat(&format!("/structure load {} {} {} {}", block_build_name(p), p.x, p.y + 1, p.z));
            }
            return held == SELECTION_WAND || held == AIR;
        }
        let me = self.player_name.clone();
        let line = match held {
            SELECTION_WAND => self.cheat_command(crate::admin::Caller::Host, "structure", &format!("save {}", block_build_name(p))),
            AIR => self.cheat_command(crate::admin::Caller::Host, "structure", &format!("load {} {} {} {}", block_build_name(p), p.x, p.y + 1, p.z)),
            _ => return false,
        };
        let _ = me;
        for l in line {
            self.msg(l);
        }
        self.player.swing = 1.0;
        true
    }

    /// `/pos1` and `/pos2` [x y z]: set a selection corner (where you stand, by default).
    pub fn cmd_pos(&mut self, me: &str, k: usize, args: &[&str], here: IVec3) -> Vec<String> {
        if me.is_empty() {
            return vec!["The console has no selection.".into()];
        }
        let p = match args {
            [] => here,
            [x, y, z] => match (crate::cheats::coord_i(x, here.x), crate::cheats::coord_i(y, here.y), crate::cheats::coord_i(z, here.z)) {
                (Some(x), Some(y), Some(z)) => ivec3(x, y, z),
                _ => return vec![format!("Usage: pos{} [x y z]", k + 1)],
            },
            _ => return vec![format!("Usage: pos{} [x y z]", k + 1)],
        };
        vec![self.set_corner(me, k, p)]
    }

    /// The caller's two corners, or an explanation.
    fn selected(&self, me: &str) -> Result<(IVec3, IVec3), String> {
        match self.selection(me) {
            [Some(a), Some(b)] => Ok((a, b)),
            _ => Err("Select something first: /pos1 and /pos2, or the Selection Wand.".into()),
        }
    }

    /// `/fill [x1 y1 z1 x2 y2 z2] <block> [hollow | outline | keep | replace <block>]`.
    pub fn cmd_fill(&mut self, me: &str, args: &[&str], here: IVec3) -> Vec<String> {
        const USAGE: &str = "Usage: fill [x1 y1 z1 x2 y2 z2] <block> [hollow | outline | keep | replace <block>]";
        let coords: Vec<Option<i32>> = args.iter().take(6).enumerate().map(|(i, a)| crate::cheats::coord_i(a, [here.x, here.y, here.z][i % 3])).collect();
        let (a, b, rest) = if args.len() > 6 && coords.iter().all(|c| c.is_some()) {
            let c: Vec<i32> = coords.into_iter().flatten().collect();
            (ivec3(c[0], c[1], c[2]), ivec3(c[3], c[4], c[5]), &args[6..])
        } else {
            match self.selected(me) {
                Ok((a, b)) => (a, b, args),
                Err(e) => return vec![e],
            }
        };
        let Some((block_name, mode)) = rest.split_first() else { return vec![USAGE.into()] };
        let block_of = |n: &str| if n.eq_ignore_ascii_case("air") { Some(AIR) } else { crate::cheats::find_item(n).filter(|&i| i < FIRST_ITEM) };
        let Some(id) = block_of(block_name) else { return vec![format!("No block called \"{block_name}\".")] };
        let mode: Vec<String> = mode.iter().map(|m| m.to_ascii_lowercase()).collect();
        let only = match mode.as_slice() {
            [m, n] if m == "replace" => match block_of(n) {
                Some(b) => Some(b),
                None => return vec![format!("No block called \"{n}\".")],
            },
            _ => None,
        };
        let shell = matches!(mode.first().map(String::as_str), Some("hollow" | "outline"));
        let hollow = mode.first().is_some_and(|m| m == "hollow");
        let keep = mode.first().is_some_and(|m| m == "keep");
        if !mode.is_empty() && !shell && !keep && only.is_none() {
            return vec![USAGE.into()];
        }
        if volume(a, b) > MAX_VOLUME {
            return vec![format!("That's {} blocks; /fill does at most {MAX_VOLUME} at once.", volume(a, b))];
        }
        let (lo, hi) = corners(a, b);
        let (lo, hi) = (lo.max(ivec3(i32::MIN, 1, i32::MIN)), hi.min(ivec3(i32::MAX, crate::world::CH - 1, i32::MAX)));
        let mut n = 0;
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                for x in lo.x..=hi.x {
                    let p = ivec3(x, y, z);
                    let edge = x == lo.x || x == hi.x || y == lo.y || y == hi.y || z == lo.z || z == hi.z;
                    let old = self.world.get_v(p);
                    let new = if shell && !edge {
                        if hollow { AIR } else { continue }
                    } else {
                        id
                    };
                    if (keep && old != AIR) || only.is_some_and(|o| o != old) || old == new {
                        continue;
                    }
                    self.world.set_v(p, new);
                    n += 1;
                }
            }
        }
        if n >= 1000 {
            self.advance_for(&crate::players::record_key(me), "big_fill");
        }
        vec![format!("Filled {n} block{}.", if n == 1 { "" } else { "s" })]
    }

    /// `/clone [x1 y1 z1 x2 y2 z2] <x y z>`: copy the selection (or box) so its low corner lands at x y z.
    pub fn cmd_clone(&mut self, me: &str, args: &[&str], here: IVec3) -> Vec<String> {
        const USAGE: &str = "Usage: clone [x1 y1 z1 x2 y2 z2] <x y z>";
        let c: Vec<Option<i32>> = args.iter().enumerate().map(|(i, a)| crate::cheats::coord_i(a, [here.x, here.y, here.z][i % 3])).collect();
        if c.iter().any(|v| v.is_none()) {
            return vec![USAGE.into()];
        }
        let c: Vec<i32> = c.into_iter().flatten().collect();
        let (a, b, to) = match c.as_slice() {
            [x, y, z] => match self.selected(me) {
                Ok((a, b)) => (a, b, ivec3(*x, *y, *z)),
                Err(e) => return vec![e],
            },
            [a, b, c2, d, e, f, x, y, z] => (ivec3(*a, *b, *c2), ivec3(*d, *e, *f), ivec3(*x, *y, *z)),
            _ => return vec![USAGE.into()],
        };
        if volume(a, b) > MAX_VOLUME {
            return vec![format!("That's {} blocks; /clone does at most {MAX_VOLUME} at once.", volume(a, b))];
        }
        let build = capture(|p| self.world.get_v(p), a, b);
        let n = self.paste_build(&build, to);
        vec![format!("Cloned {} blocks ({n} changed).", build.blocks.len())]
    }

    /// `/structure save|load|export|list|delete`.
    pub fn cmd_structure(&mut self, me: &str, args: &[&str], here: IVec3) -> Vec<String> {
        const USAGE: &str = "Usage: structure save <name> | load <name> [x y z] | export <name> | list | delete <name>";
        let name = args.get(1).and_then(|n| clean_name(n));
        match (args.first().map(|a| a.to_ascii_lowercase()).as_deref(), name) {
            (Some("list"), _) => {
                if self.builds.is_empty() {
                    vec!["No saved builds yet: /structure save <name>.".into()]
                } else {
                    vec![format!("Saved builds: {}", self.builds.keys().cloned().collect::<Vec<_>>().join(", "))]
                }
            }
            (Some("save"), Some(n)) => {
                let (a, b) = match self.selected(me) {
                    Ok(s) => s,
                    Err(e) => return vec![e],
                };
                if volume(a, b) > MAX_VOLUME {
                    return vec![format!("That's {} blocks; a build is at most {MAX_VOLUME}.", volume(a, b))];
                }
                let build = capture(|p| self.world.get_v(p), a, b);
                match to_template(&n, &build) {
                    Ok(t) => {
                        self.builds.insert(n.clone(), t);
                        self.advance_for(&crate::players::record_key(me), "saved_for_later");
                        vec![format!("Saved {n}: {}x{}x{}, {} blocks.", build.size.x, build.size.y, build.size.z, build.solid())]
                    }
                    Err(e) => vec![format!("Can't save that: {e}.")],
                }
            }
            (Some("load"), Some(n)) => {
                let Some(text) = self.builds.get(&n).cloned() else { return vec![format!("No build called {n} (see /structure list).")] };
                let at = match &args[2..] {
                    [] => here,
                    [x, y, z] => match (crate::cheats::coord_i(x, here.x), crate::cheats::coord_i(y, here.y), crate::cheats::coord_i(z, here.z)) {
                        (Some(x), Some(y), Some(z)) => ivec3(x, y, z),
                        _ => return vec![USAGE.into()],
                    },
                    _ => return vec![USAGE.into()],
                };
                match from_template(&text) {
                    Ok(b) => {
                        let changed = self.paste_build(&b, at);
                        vec![format!("Pasted {n} at {}, {}, {} ({changed} blocks changed).", at.x, at.y, at.z)]
                    }
                    Err(e) => vec![format!("{n} is damaged: {e}.")],
                }
            }
            (Some("export"), Some(n)) => {
                let Some(text) = self.builds.get(&n).cloned() else { return vec![format!("No build called {n} (see /structure list).")] };
                let dir = std::path::Path::new("exports");
                let path = dir.join(format!("{n}.txt"));
                let note = match from_template(&text) {
                    Ok(b) if b.size.max_element() > 32 => " (mod structures are at most 32 blocks each way, so trim it before using it in a mod)",
                    _ => "",
                };
                match std::fs::create_dir_all(dir).and_then(|_| std::fs::write(&path, &text)) {
                    Ok(()) => vec![format!("Wrote {} as a mod structure template{note}. Paste it into a mod.txt.", path.display())],
                    Err(e) => vec![format!("Couldn't write {}: {e}.", path.display())],
                }
            }
            (Some("delete"), Some(n)) => {
                if self.builds.remove(&n).is_some() {
                    vec![format!("Deleted {n}.")]
                } else {
                    vec![format!("No build called {n}.")]
                }
            }
            _ => vec![USAGE.into()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admin::Caller;

    #[test]
    fn mirror_building_copies_placing_and_breaking() {
        let mut g = crate::game::tests::arena(842);
        g.creative = false;
        assert_eq!(g.symmetry_command("/symmetry x").unwrap()[0], "Mirror building is for creative mode.");
        g.creative = true;
        let c = g.player.body.pos.floor().as_ivec3();
        assert!(g.symmetry_command("/symmetry both").unwrap()[0].starts_with("Mirror building across x and z"));
        let p = c + ivec3(2, 1, 3);
        let stairs = stairs(0, 1);
        g.world.set_v(p, stairs);
        g.place_mirrored(p, stairs);
        assert_eq!(g.world.get_v(c + ivec3(-2, 1, 3)), stairs - 1 + 3, "facing west in the mirror");
        assert_eq!(g.world.get_v(c + ivec3(2, 1, -3)), stairs, "east still, across z");
        assert_eq!(g.world.get_v(c + ivec3(-2, 1, -3)), stairs - 1 + 3);
        assert!(g.advancements.has("mirror_mirror"));
        assert_eq!(g.mirror_spots(p).len(), 3);
        assert!(g.symmetry_command("/symmetry").unwrap()[0].contains("off"));
        assert!(g.mirror_spots(p).is_empty());
        assert!(g.symmetry_command("/symmetryish").is_none());
    }

    #[test]
    fn the_ghost_shows_where_a_block_would_go() {
        let mut g = crate::game::tests::arena(843);
        g.creative = true;
        let floor = ivec3(3, 49, 0);
        g.world.set_v(floor, STONE);
        g.target = Some(crate::game::Target::Block(crate::world::Hit { pos: floor, normal: IVec3::Y, dist: 2.0 }));
        g.inv.slots[g.inv.selected] = Some((BRICK, 1));
        assert_eq!(g.ghost_spots(), Some(vec![floor + IVec3::Y]));
        g.ghost_preview = false;
        assert_eq!(g.ghost_spots(), None, "it can be turned off");
        g.ghost_preview = true;
        g.creative = false;
        assert_eq!(g.ghost_spots(), None, "creative only");
    }

    fn run(g: &mut Game, line: &str) -> Vec<String> {
        g.admin_command(Caller::Host, line).unwrap()
    }

    #[test]
    fn templates_round_trip() {
        let b = Build { size: ivec3(2, 2, 1), blocks: vec![STONE, AIR, GLASS, crate::stonesets::wall(0).unwrap()] };
        let t = to_template("hut", &b).unwrap();
        assert!(t.starts_with("[structure hut]"));
        assert_eq!(from_template(&t).unwrap(), b);
        let mut m = BTreeMap::new();
        m.insert("hut".to_string(), t);
        assert_eq!(decode(&encode(&m)), m);
    }

    #[test]
    fn fill_clone_and_saved_builds() {
        let mut g = crate::game::tests::arena(840);
        g.creative = true;
        let me = g.player_name.clone();
        let base = ivec3(4, 52, 4);
        assert!(run(&mut g, "/fill stone")[0].starts_with("Select something first"));
        run(&mut g, &format!("/pos1 {} {} {}", base.x, base.y, base.z));
        run(&mut g, &format!("/pos2 {} {} {}", base.x + 2, base.y + 2, base.z + 2));
        assert_eq!(g.selection(&me), [Some(base), Some(base + IVec3::splat(2))]);
        assert_eq!(g.my_selection[1], Some(base + IVec3::splat(2)), "the host sees its own selection");
        // A hollow cube of glass: 26 of 27 blocks.
        let out = run(&mut g, "/fill glass hollow");
        assert_eq!(out[0], "Filled 26 blocks.");
        assert_eq!(g.world.get_v(base + IVec3::ONE), AIR);
        assert_eq!(g.world.get_v(base), GLASS);
        // Swap the glass for stone, only where it's glass.
        run(&mut g, "/fill stone replace glass");
        assert_eq!(g.world.get_v(base + IVec3::X), STONE);
        assert_eq!(g.world.get_v(base + IVec3::ONE), AIR);
        // Clone it alongside.
        let to = base + ivec3(5, 0, 0);
        run(&mut g, &format!("/clone {} {} {}", to.x, to.y, to.z));
        assert_eq!(g.world.get_v(to + IVec3::Z), STONE);
        assert_eq!(g.world.get_v(to + IVec3::ONE), AIR);
        // Save it, paste it somewhere else, and it's saved with the world.
        assert!(run(&mut g, "/structure save Box!")[0].starts_with("Saved box: 3x3x3, 26 blocks"));
        assert!(g.advancements.has("saved_for_later"));
        let far = base + ivec3(0, 0, 8);
        run(&mut g, &format!("/structure load box {} {} {}", far.x, far.y, far.z));
        assert_eq!(g.world.get_v(far + ivec3(2, 2, 2)), STONE);
        let saved = encode(&g.builds);
        assert_eq!(decode(&saved).len(), 1);
        // Too big is refused.
        assert!(run(&mut g, "/fill 0 1 0 200 100 200 stone")[0].contains("at most"));
    }

    #[test]
    fn the_wand_and_the_structure_block() {
        let mut g = crate::game::tests::arena(841);
        g.creative = true;
        let a = ivec3(3, 50, 3);
        g.world.set_v(a, BRICK);
        g.wand_corner(0, a);
        g.wand_corner(1, a + IVec3::Y);
        let sb = ivec3(-4, 50, -4);
        g.world.set_v(sb, STRUCTURE_BLOCK);
        g.inv.slots[g.inv.selected] = Some((SELECTION_WAND, 1));
        assert!(g.use_structure_block(sb));
        assert!(g.builds.contains_key(&block_build_name(sb)));
        g.inv.slots[g.inv.selected] = None;
        assert!(g.use_structure_block(sb));
        assert_eq!(g.world.get_v(sb + IVec3::Y), BRICK, "pasted on top");
    }
}
