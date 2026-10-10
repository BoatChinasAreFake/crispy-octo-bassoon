//! Saving the dimensions (see realms.rs). The world file holds the Overworld
//! as it always has; each other dimension that's been visited gets a section
//! of its own ("realm1", "realm2") packed with the same encoders, and "dim"
//! says which one the player is in. With region files, their edits, chests,
//! soil and decor live in a folder per dimension (see regions.rs) instead.
//!
//! A save from before the dimensions were separated ("dims" missing) had them
//! all on one map, far apart in x. Loading it sorts everything out by where it
//! was (`Dim::of_old_x`) and moves it to the dimension's own coordinates; its
//! region files are rewritten the same way the first time they're opened.

use crate::block::{Id, AIR};
use crate::dims::Dim;
use crate::game::Game;
use crate::realms::Realm;
use macroquad::math::{IVec3, Vec3};
use std::collections::HashMap;

type Mods = HashMap<(i32, i32), HashMap<u32, Id>>;

fn put(out: &mut Vec<u8>, key: &str, b: &[u8]) {
    out.push(key.len() as u8);
    out.extend_from_slice(key.as_bytes());
    out.extend_from_slice(&(b.len() as u32).to_le_bytes());
    out.extend_from_slice(b);
}

/// `put`'s sections back (stops at anything malformed).
fn sections(mut b: &[u8]) -> Vec<(String, &[u8])> {
    let mut v = Vec::new();
    while let Some(&n) = b.first() {
        let n = n as usize;
        let Some(key) = b.get(1..1 + n) else { break };
        let Some(len) = b.get(1 + n..5 + n).map(|s| u32::from_le_bytes(s.try_into().unwrap()) as usize) else { break };
        let Some(body) = b.get(5 + n..5 + n + len) else { break };
        v.push((String::from_utf8_lossy(key).into_owned(), body));
        b = &b[5 + n + len..];
    }
    v
}

fn encode_mods(mods: &Mods) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(mods.len() as u32).to_le_bytes());
    for (&(cx, cz), m) in mods {
        out.extend_from_slice(&cx.to_le_bytes());
        out.extend_from_slice(&cz.to_le_bytes());
        out.extend_from_slice(&(m.len() as u32).to_le_bytes());
        for (&i, &id) in m {
            out.extend_from_slice(&i.to_le_bytes());
            out.extend_from_slice(&id.to_le_bytes());
        }
    }
    out
}

fn decode_mods(b: &[u8]) -> Mods {
    let mut r = crate::save::R(b);
    let mut mods = Mods::new();
    let Ok(n) = r.u32() else { return mods };
    for _ in 0..n.min(1 << 20) {
        let (Ok(cx), Ok(cz), Ok(k)) = (r.i32(), r.i32(), r.u32()) else { break };
        let mut m = HashMap::new();
        for _ in 0..k.min(1 << 20) {
            let (Ok(i), Ok(id)) = (r.u32(), r.u16()) else { return mods };
            m.insert(i, id);
        }
        mods.insert((cx, cz), m);
    }
    mods
}

/// From the old one-map layout: which dimension a block was in, and where it is in that one's own.
fn sort_out(p: IVec3) -> (Dim, IVec3) {
    let d = Dim::of_old_x(p.x);
    (d, p - IVec3::new(d.gen_x(), 0, 0))
}

fn sort_out_v(p: Vec3) -> (Dim, Vec3) {
    let d = Dim::of_old_x(p.x.floor() as i32);
    (d, p - Vec3::new(d.gen_x() as f32, 0.0, 0.0))
}

impl Game {
    /// The active dimension's own things, packed (see the top of this file).
    pub fn realm_part(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let w = &self.world;
        if w.regions.is_none() {
            put(&mut out, "mods", &encode_mods(&w.mods));
            put(&mut out, "farm", &crate::farming::encode(&w.farm));
            put(&mut out, "containers", &crate::containers::encode(&w.containers));
            put(&mut out, "decor", &crate::decor::encode(&w.signs, &w.frames));
        }
        put(&mut out, "sign_styles", &crate::qol::encode_styles(&w.sign_styles));
        put(&mut out, "drops", &crate::drops::encode(&self.drops));
        put(&mut out, "mobs", &crate::animals::encode_mobs(&self.mobs, &self.mob_names));
        put(&mut out, "vehicles", &crate::vehicles::encode(&self.vehicles));
        put(&mut out, "hives", &crate::bees::encode(&self.hives));
        put(&mut out, "pots", &crate::cooking::encode(&self.pots));
        put(&mut out, "banners", &crate::banners::encode(&self.banners));
        put(&mut out, "lecterns", &crate::books::encode(&HashMap::new(), &self.lecterns));
        put(&mut out, "packs", &self.encode_packs());
        out
    }

    /// Unpack `realm_part` into the active dimension (`remap`: the save's
    /// block and item ids to today's, if mods changed).
    pub fn load_realm_part(&mut self, b: &[u8], remap: Option<&[Id]>) {
        let map = |id: Id| remap.map(|r| r[id as usize]).unwrap_or(id);
        let mut packs = None;
        for (key, body) in sections(b) {
            match key.as_str() {
                "mods" => {
                    self.world.mods = decode_mods(body);
                    for m in self.world.mods.values_mut() {
                        for id in m.values_mut() {
                            *id = map(*id);
                        }
                    }
                }
                "farm" => self.world.farm = crate::farming::decode(body),
                "containers" => {
                    for (p, mut c) in crate::containers::decode(body, 4) {
                        for s in c.slots.iter_mut() {
                            *s = s.map(|(id, n)| (map(id), n)).filter(|(id, _)| *id != AIR);
                        }
                        self.world.containers.insert(p, c);
                    }
                }
                "decor" => {
                    let (signs, frames) = crate::decor::decode(body);
                    self.world.signs = signs;
                    self.world.frames = frames.into_iter().map(|(p, (item, wear))| (p, (map(item), wear))).filter(|(_, (item, _))| *item != AIR).collect();
                }
                "sign_styles" => self.world.sign_styles = crate::qol::decode_styles(body),
                "drops" => {
                    for (pos, item, n, age, wear) in crate::drops::decode(body, 4) {
                        self.spawn_drop(pos, map(item), n, wear, Vec3::ZERO, 0.0);
                        if let Some(last) = self.drops.last_mut() {
                            last.age = age;
                        }
                    }
                }
                "mobs" => {
                    for (mut m, name) in crate::animals::decode_mobs_named(body, &mut self.rng) {
                        m.id = self.next_mob_id;
                        self.next_mob_id += 1;
                        if let Some(name) = name {
                            self.mob_names.insert(m.id, name);
                        }
                        self.mobs.push(m);
                    }
                }
                "vehicles" => {
                    for (kind, pos, yaw, cargo) in crate::vehicles::decode(body) {
                        let id = self.spawn_vehicle(kind, pos, yaw);
                        if let (Some(mut c), Some(v)) = (cargo, self.vehicles.iter_mut().find(|v| v.id == id)) {
                            for s in c.slots.iter_mut() {
                                *s = s.map(|(id, n)| (map(id), n)).filter(|(id, _)| *id != AIR);
                            }
                            if v.contents.as_ref().is_some_and(|have| have.slots.len() == c.slots.len()) {
                                v.contents = Some(c);
                            }
                        }
                    }
                }
                "hives" => self.hives = crate::bees::decode(body),
                "pots" => self.pots = crate::cooking::decode(body),
                "banners" => self.banners = crate::banners::decode(body),
                "lecterns" => self.lecterns = crate::books::decode(body).1,
                "packs" => packs = Some(body),
                _ => {}
            }
        }
        // (After the mobs, whose packs they are.)
        if let Some(b) = packs {
            self.decode_packs(b);
        }
    }

    /// The save sections for the dimensions (the Overworld must be active:
    /// it's what the rest of the save holds).
    pub fn save_realms(&mut self) -> Vec<(String, Vec<u8>)> {
        let mut v = vec![("dims".to_string(), vec![1]), ("dim".to_string(), vec![self.dim.index()])];
        for d in [Dim::Scorch, Dim::Hollow] {
            if self.parked.contains_key(&d) {
                let b = self.in_realm(d, |g| g.realm_part());
                v.push((format!("realm{}", d.index()), b));
            }
        }
        v
    }

    /// Loading: the other dimensions' sections, and which one the player is in.
    pub fn load_realms(&mut self, extras: &[(String, Vec<u8>)], remap: Option<&[Id]>) {
        if !crate::realms::is_split(extras) {
            self.split_old_layout();
            return;
        }
        for d in [Dim::Scorch, Dim::Hollow] {
            let key = format!("realm{}", d.index());
            if let Some((_, b)) = extras.iter().find(|(k, _)| *k == key) {
                self.in_realm(d, |g| g.load_realm_part(b, remap));
            }
        }
        let dim = extras.iter().find(|(k, _)| k == "dim").and_then(|(_, b)| b.first()).and_then(|&d| Dim::from_index(d)).unwrap_or_default();
        self.enter(dim);
        self.dim = dim;
    }

    /// A save from when the dimensions shared one map (only the Overworld is
    /// loaded so far): move everything to the dimension it was in.
    fn split_old_layout(&mut self) {
        self.old_layout = true;
        let mut parts: HashMap<Dim, Realm> = HashMap::new();
        let opts = self.world.generator.opts;
        for d in [Dim::Scorch, Dim::Hollow] {
            parts.insert(d, Realm::new(d, &self.world, opts));
        }
        macro_rules! split_map {
            ($($f:ident).+) => {{
                let all = std::mem::take(&mut self.$($f).+);
                for (p, v) in all {
                    match sort_out(p) {
                        (Dim::Over, _) => {
                            self.$($f).+.insert(p, v);
                        }
                        (d, q) => {
                            parts.get_mut(&d).unwrap().$($f).+.insert(q, v);
                        }
                    }
                }
            }};
        }
        split_map!(world.farm);
        split_map!(world.containers);
        split_map!(world.signs);
        split_map!(world.frames);
        split_map!(world.sign_styles);
        split_map!(hives);
        split_map!(banners);
        split_map!(lecterns);
        for ((cx, cz), m) in std::mem::take(&mut self.world.mods) {
            match Dim::of_old_x(cx * crate::world::CW + 8) {
                Dim::Over => {
                    self.world.mods.insert((cx, cz), m);
                }
                d => {
                    parts.get_mut(&d).unwrap().world.mods.insert((cx - d.gen_cx(), cz), m);
                }
            }
        }
        for mut m in std::mem::take(&mut self.mobs) {
            let (d, at) = sort_out_v(m.body.pos);
            if d == Dim::Over {
                self.mobs.push(m);
                continue;
            }
            let shift = Vec3::new(d.gen_x() as f32, 0.0, 0.0);
            m.body.pos = at;
            m.net_pos -= shift;
            m.goal = None;
            m.home = m.home.map(|h| h - shift);
            if let Some(pack) = self.world.packs.remove(&m.id) {
                parts.get_mut(&d).unwrap().world.packs.insert(m.id, pack);
            }
            parts.get_mut(&d).unwrap().mobs.push(m);
        }
        for mut item in std::mem::take(&mut self.drops) {
            let (d, at) = sort_out_v(item.body.pos);
            item.net_pos -= item.body.pos - at;
            item.body.pos = at;
            match d {
                Dim::Over => self.drops.push(item),
                d => parts.get_mut(&d).unwrap().drops.push(item),
            }
        }
        for mut v in std::mem::take(&mut self.vehicles) {
            let (d, at) = sort_out_v(v.pos);
            v.pos = at;
            match d {
                Dim::Over => self.vehicles.push(v),
                d => parts.get_mut(&d).unwrap().vehicles.push(v),
            }
        }
        for (d, r) in parts {
            self.parked.insert(d, r);
        }
        let (dim, at) = sort_out_v(self.player.body.pos);
        self.enter(dim);
        self.dim = dim;
        self.player.body.pos = at;
        self.player.fall_start = at.y;
    }

    /// Keep every dimension's edits in region files under `dir` (a folder
    /// each; see regions.rs). An old save's files are sorted out first.
    pub fn use_regions(&mut self, dir: std::path::PathBuf) {
        if std::mem::take(&mut self.old_layout)
            && let Err(e) = crate::regions::split_old_regions(&dir)
        {
            self.msg(format!("Couldn't sort out the old region files: {e}"));
        }
        self.world.use_regions(dir.clone());
        for r in self.parked.values_mut() {
            r.world.use_regions(dir.clone());
        }
    }

    /// Write every dimension's region files.
    pub fn flush_regions(&mut self) -> std::io::Result<()> {
        let mut result = self.world.flush_regions();
        for r in self.parked.values_mut() {
            result = result.and(r.world.flush_regions());
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::*;
    use macroquad::math::ivec3;

    #[test]
    fn sections_and_mods_round_trip() {
        let mut out = Vec::new();
        put(&mut out, "a", b"xyz");
        put(&mut out, "bee", b"");
        let s = sections(&out);
        assert_eq!(s, vec![("a".to_string(), &b"xyz"[..]), ("bee".to_string(), &b""[..])]);
        let mut mods = Mods::new();
        mods.insert((-3, 7), HashMap::from([(5, STONE), (99, TORCH)]));
        assert_eq!(decode_mods(&encode_mods(&mods)), mods);
    }

    #[test]
    fn realms_survive_a_save() {
        let mut g = crate::game::tests::arena(5);
        g.in_realm(Dim::Scorch, |g| {
            g.world.signs.insert(ivec3(2, 40, 3), ["hot".into(), "".into(), "".into(), "".into()]);
            g.alloc_mob(crate::entity::MobKind::Grumbler, Vec3::new(3.0, 40.0, 3.0));
            g.mobs.last_mut().unwrap().persistent = true;
        });
        g.move_local_player(Dim::Scorch, Vec3::new(1.0, 41.0, 1.0));
        let d = g.to_save();
        let mut back = Game::from_save(d);
        assert_eq!(back.dim, Dim::Scorch);
        assert_eq!(back.realm_dim(), Dim::Scorch);
        assert_eq!(back.player.body.pos, Vec3::new(1.0, 41.0, 1.0));
        assert_eq!(back.world.signs[&ivec3(2, 40, 3)][0], "hot");
        assert!(back.mobs.iter().any(|m| m.kind == crate::entity::MobKind::Grumbler));
        assert!(back.in_realm(Dim::Over, |g| !g.world.signs.contains_key(&ivec3(2, 40, 3))));
    }

    #[test]
    fn old_one_map_saves_are_sorted_out() {
        let mut g = crate::game::tests::arena(5);
        let far = crate::scorch::SCORCH_ORIGIN;
        g.world.signs.insert(ivec3(far + 2, 40, 3), ["hot".into(), "".into(), "".into(), "".into()]);
        g.world.signs.insert(ivec3(2, 60, 3), ["home".into(), "".into(), "".into(), "".into()]);
        g.world.mods.insert((far / 16, 0), HashMap::from([(1, SCORCHROCK)]));
        g.alloc_mob(crate::entity::MobKind::Grumbler, Vec3::new(far as f32 + 3.0, 40.0, 3.0));
        g.mobs.last_mut().unwrap().persistent = true;
        g.player.body.pos = Vec3::new(far as f32 + 1.0, 41.0, 1.0);
        g.split_old_layout();
        assert_eq!(g.dim, Dim::Scorch);
        assert_eq!(g.player.body.pos, Vec3::new(1.0, 41.0, 1.0));
        assert_eq!(g.world.signs[&ivec3(2, 40, 3)][0], "hot");
        assert_eq!(g.world.mods[&(0, 0)][&1], SCORCHROCK);
        assert!(g.mobs.iter().any(|m| m.kind == crate::entity::MobKind::Grumbler && m.body.pos.x < 10.0));
        g.enter(Dim::Over);
        assert_eq!(g.world.signs[&ivec3(2, 60, 3)][0], "home");
        assert_eq!(g.world.signs.len(), 1);
    }
}
