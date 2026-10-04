//! Who's who: skins for players, and Name Tags for mobs.
//!
//! - **Skins.** Options > Skin picks one of six looks (the classic Stove and
//!   friends). It's saved in settings.txt and everyone else sees it too.
//! - **Name Tags** (a string and a book make two). Right-click a mob with
//!   one, type a name, press Enter: the name floats above it, and a named mob
//!   is never cleared away (it's saved with the world). Joined players name
//!   mobs through the host, which checks they had the tag.

use crate::block::*;
use crate::entity::{humanoid, Limb, Part};
use crate::game::Game;
use crate::net::Msg;
use crate::texture::T_SKIN_FIRST;

/// A skin: name, skin tone, hair, shirt, trousers.
pub type Skin = (&'static str, [u8; 3], [u8; 3], [u8; 3], [u8; 3]);

/// The skins: name, skin tone, hair, shirt, trousers.
pub const SKINS: [Skin; 6] = [
    ("Stove", [200, 150, 110], [60, 40, 20], [60, 170, 170], [60, 60, 150]),
    ("Alexa", [230, 185, 150], [200, 100, 40], [80, 160, 70], [110, 80, 50]),
    ("Kettle", [140, 95, 65], [25, 20, 20], [200, 60, 60], [50, 50, 60]),
    ("Toaster", [245, 205, 175], [230, 210, 120], [240, 200, 60], [60, 90, 160]),
    ("Blender", [110, 75, 50], [40, 30, 25], [140, 80, 180], [40, 40, 40]),
    ("Fridge", [220, 170, 130], [150, 150, 155], [235, 235, 240], [90, 90, 100]),
];

/// Tiles of skin `k`: tone, face, shirt, trousers.
pub const fn skin_tiles(k: u16) -> [u16; 4] {
    let b = T_SKIN_FIRST + k * 4;
    [b, b + 1, b + 2, b + 3]
}

const fn skin_model(k: u16) -> [Part; 6] {
    let t = skin_tiles(k);
    humanoid(t[0], t[1], t[2], t[3], Limb::Swing(-1.0), Limb::Swing(1.0))
}

/// Players' bodies, one per skin.
pub static SKIN_MODELS: [[Part; 6]; 6] = [skin_model(0), skin_model(1), skin_model(2), skin_model(3), skin_model(4), skin_model(5)];

pub fn skin_name(k: u8) -> &'static str {
    SKINS[k as usize % SKINS.len()].0
}

/// Longest mob name.
pub const NAME_LEN: usize = 24;

/// Tidy a name: printable, trimmed, not too long.
pub fn clean_name(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(NAME_LEN).collect::<String>().trim().to_string()
}

impl Game {
    /// Change how we look, and tell the other players.
    pub fn set_skin(&mut self, k: u8) {
        self.skin = k % SKINS.len() as u8;
        if self.net.is_some() {
            let m = Msg::PlayerSkin { id: self.my_id, skin: self.skin };
            self.net_send_msg(m);
        }
    }

    /// Give mob `id` a name (the local player, holding a Name Tag).
    pub fn name_mob(&mut self, id: u32, name: &str) {
        let name = clean_name(name);
        if name.is_empty() || self.inv.held() != NAME_TAG {
            return;
        }
        if self.is_client() {
            self.net_send_msg(Msg::MobName { mob: id, name });
            // The host takes the tag (our ledger follows); take it here too.
            if !self.creative {
                self.inv.consume_held();
            }
            return;
        }
        if !self.creative {
            self.use_up_held();
        }
        self.set_mob_name(id, name);
    }

    /// Where the world lives: name a mob and tell everyone.
    pub fn set_mob_name(&mut self, id: u32, name: String) {
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) else { return };
        m.persistent = true;
        self.mob_names.insert(id, name.clone());
        self.net_broadcast(Msg::MobName { mob: id, name });
        self.advance("hello_my_name_is");
    }

    /// A joined player named a mob.
    pub fn host_mob_name(&mut self, from: u32, mob: u32, name: String) {
        let name = clean_name(&name);
        let near = match (self.peers.get(&from), self.mobs.iter().find(|m| m.id == mob)) {
            (Some(p), Some(m)) => p.target.distance(m.body.pos) < 8.0,
            _ => false,
        };
        if !name.is_empty() && near && (self.creative || self.peer_take(from, NAME_TAG, 1)) {
            self.set_mob_name(mob, name);
        }
    }

    /// Forget the names of mobs that are gone.
    pub fn tidy_mob_names(&mut self) {
        if self.is_client() {
            return;
        }
        let alive: std::collections::HashSet<u32> = self.mobs.iter().map(|m| m.id).collect();
        self.mob_names.retain(|id, _| alive.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Mob, MobKind};
    use macroquad::math::Vec3;

    #[test]
    fn naming_a_mob_keeps_it() {
        let mut g = crate::game::tests::arena(111);
        let mut m = Mob::new(MobKind::Oinker, Vec3::new(2.5, 50.0, 0.5), &mut g.rng);
        m.id = 55;
        g.mobs.push(m);
        g.inv.slots[g.inv.selected] = Some((NAME_TAG, 1));
        g.name_mob(55, "  Sir Oinks\u{7}alot  ");
        assert_eq!(g.mob_names.get(&55).map(String::as_str), Some("Sir Oinksalot"));
        assert!(g.mobs[0].persistent);
        assert_eq!(g.inv.count(NAME_TAG), 0);
        // And it's saved.
        let bytes = crate::animals::encode_mobs(&g.mobs, &g.mob_names);
        let back = crate::animals::decode_mobs_named(&bytes, &mut g.rng);
        assert_eq!(back[0].1.as_deref(), Some("Sir Oinksalot"));
        assert_eq!(clean_name(&"x".repeat(50)).len(), NAME_LEN);
    }
}
