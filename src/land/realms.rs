//! **Realms**: one dimension's world and everything in it (see dims.rs).
//!
//! The game works on one realm at a time, the *active* one, kept in the
//! usual `Game` fields (`world`, `mobs`, `drops`, `peers` and the rest), so
//! everything written before there were separate dimensions just works on
//! "the world". The others wait in `Game::parked`. Changing dimension, or the
//! host looking after players somewhere else, swaps a parked realm in
//! (`enter`): only pointers move.
//!
//! The local player isn't part of any realm; `Game::player` is wherever
//! `Game::dim` says. While a realm the local player isn't in is active (the
//! host ticking another dimension for the players there), the game counts as
//! having no local player (`away`).

use crate::block::Id;
use crate::dims::Dim;
use crate::game::Game;
use crate::multiplayer::Peer;
use crate::world::{GenOptions, World};
use macroquad::math::{IVec3, Vec3};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Lists every field a realm owns (name: type), for the struct and the swap.
macro_rules! realm_fields {
    ($m:ident) => {
        $m! {
            world: World,
            mobs: Vec<crate::entity::Mob>,
            particles: Vec<crate::entity::Particle>,
            tnts: Vec<crate::entity::PrimedTnt>,
            falling: Vec<crate::falling::FallingBlock>,
            powered_notes: HashSet<IVec3>,
            fireballs: Vec<crate::fortress::Fireball>,
            trials: HashMap<IVec3, crate::trial::Trial>,
            raid: Option<crate::raids::Raid>,
            bell_glow: f32,
            arrows: Vec<crate::entity::Arrow>,
            sounds: Vec<(crate::sound::Sfx, Option<Vec3>)>,
            dig_queue: Vec<IVec3>,
            cave_ins: Vec<crate::caveins::CaveIn>,
            refill: Vec<IVec3>,
            peers: BTreeMap<u32, Peer>,
            viewers: HashMap<IVec3, HashSet<u32>>,
            dirty_containers: HashSet<IVec3>,
            detectors: HashMap<IVec3, f32>,
            drops: Vec<crate::drops::ItemDrop>,
            orbs: Vec<crate::xp::XpOrb>,
            decaying: Vec<(IVec3, f32)>,
            dispensers_on: HashSet<IVec3>,
            observed: HashMap<IVec3, Id>,
            observer_pulses: HashMap<IVec3, f32>,
            bastions_peopled: HashSet<IVec3>,
            frosted: Vec<(IVec3, f32)>,
            lava_waiting: HashSet<IVec3>,
            buttons: HashMap<IVec3, f32>,
            plates: HashMap<IVec3, f32>,
            powered_doors: HashSet<IVec3>,
            vehicles: Vec<crate::vehicles::Vehicle>,
            hives: HashMap<IVec3, crate::bees::Colony>,
            buzz: Vec<crate::bees::Buzz>,
            sensors_on: HashMap<IVec3, f32>,
            lids: HashMap<IVec3, crate::chests::Lid>,
            lecterns: HashMap<IVec3, (Id, u16)>,
            banners: HashMap<IVec3, (u16, u8)>,
            drops_sent_empty: bool,
            orbs_sent_empty: bool,
        }
    };
}

macro_rules! define_realm {
    ($($f:ident: $t:ty,)*) => {
        /// A dimension parked while another is active (see the top of this file).
        pub struct Realm {
            $(pub $f: $t,)*
        }

        impl Realm {
            /// Swap this realm's contents with the game's active ones.
            pub fn swap_with(&mut self, g: &mut Game) {
                $(std::mem::swap(&mut self.$f, &mut g.$f);)*
            }
        }
    };
}
realm_fields!(define_realm);

impl Realm {
    /// A fresh realm for `dim`, set up like `like` (the active world: the
    /// same seed and options, and the same rules about who simulates what).
    pub fn new(dim: Dim, like: &World, opts: GenOptions) -> Realm {
        let mut world = World::with_dim(like.seed(), opts, dim);
        world.structure_loot = like.structure_loot;
        world.simulate_liquids = like.simulate_liquids;
        world.log_edits = like.log_edits;
        world.regions = like.regions.as_ref().map(|r| r.for_dim(dim));
        Realm {
            world,
            mobs: Vec::new(),
            particles: Vec::new(),
            tnts: Vec::new(),
            falling: Vec::new(),
            powered_notes: HashSet::new(),
            fireballs: Vec::new(),
            trials: HashMap::new(),
            raid: None,
            bell_glow: 0.0,
            arrows: Vec::new(),
            sounds: Vec::new(),
            dig_queue: Vec::new(),
            cave_ins: Vec::new(),
            refill: Vec::new(),
            peers: BTreeMap::new(),
            viewers: HashMap::new(),
            dirty_containers: HashSet::new(),
            detectors: HashMap::new(),
            drops: Vec::new(),
            orbs: Vec::new(),
            decaying: Vec::new(),
            dispensers_on: HashSet::new(),
            observed: HashMap::new(),
            observer_pulses: HashMap::new(),
            bastions_peopled: HashSet::new(),
            frosted: Vec::new(),
            lava_waiting: HashSet::new(),
            buttons: HashMap::new(),
            plates: HashMap::new(),
            powered_doors: HashSet::new(),
            vehicles: Vec::new(),
            hives: HashMap::new(),
            buzz: Vec::new(),
            sensors_on: HashMap::new(),
            lids: HashMap::new(),
            lecterns: HashMap::new(),
            banners: HashMap::new(),
            drops_sent_empty: false,
            orbs_sent_empty: false,
        }
    }
}

/// Was this save written with the dimensions separated (its realms each in
/// their own section; see `Game::save_realms`)? Older ones get split up.
pub fn is_split(extras: &[(String, Vec<u8>)]) -> bool {
    extras.iter().any(|(k, _)| k == "dims")
}

impl Game {
    /// The active realm's dimension.
    pub fn realm_dim(&self) -> Dim {
        self.world.dim()
    }

    /// Is the local player away from the active realm (the host looking after
    /// another dimension, or a dedicated server)?
    pub fn away(&self) -> bool {
        self.dedicated || self.realm_dim() != self.dim
    }

    /// Make `dim`'s realm the active one (making it, if it's new).
    pub fn enter(&mut self, dim: Dim) {
        if self.realm_dim() == dim {
            return;
        }
        let mut r = match self.parked.remove(&dim) {
            Some(r) => r,
            None => Realm::new(dim, &self.world, self.world.generator.opts),
        };
        r.swap_with(self);
        // Personal Chests and backpacks are the same everywhere: they go
        // with whichever world is active.
        std::mem::swap(&mut self.world.stashes, &mut r.world.stashes);
        std::mem::swap(&mut self.world.backpacks, &mut r.world.backpacks);
        // `r` now holds what was active.
        self.parked.insert(r.world.dim(), r);
    }

    /// Run `f` with `dim`'s realm active, then put back whichever was.
    pub fn in_realm<T>(&mut self, dim: Dim, f: impl FnOnce(&mut Game) -> T) -> T {
        let was = self.realm_dim();
        self.enter(dim);
        let out = f(self);
        self.enter(was);
        out
    }

    /// Every dimension with a realm (active or parked).
    pub fn realm_dims(&self) -> Vec<Dim> {
        let mut v: Vec<Dim> = self.parked.keys().copied().collect();
        v.push(self.realm_dim());
        v.sort();
        v
    }

    /// Put the local player at `to` in `dim` (going there if need be): things
    /// held in the old dimension (a vehicle, an open chest...) are let go.
    pub fn move_local_player(&mut self, dim: Dim, to: Vec3) {
        if self.dim != dim {
            if self.is_host() {
                let id = self.my_id;
                self.net_broadcast_all(crate::net::Msg::Dimension { id, dim: dim.index(), x: to.x, y: to.y, z: to.z });
            }
            self.riding = None;
            self.mounted = None;
            self.open = None;
            self.breaking = None;
            self.sleeping = None;
            self.bobber = None;
            self.target = None;
            self.editing_sign = None;
        }
        let moved = self.dim != dim;
        self.enter(dim);
        self.dim = dim;
        if moved {
            // Everything here gets drawn afresh.
            let all: Vec<(i32, i32)> = self.world.chunks.keys().copied().collect();
            self.world.dirty.extend(all);
            self.lod = crate::lod::Lod::default();
        }
        self.player.body.pos = to;
        self.player.body.vel = Vec3::ZERO;
        self.player.fall_start = to.y;
        self.ready = false;
    }

    /// Where the world lives: a joined player goes to `to` in `dim`, which is
    /// left active. Everyone is told (so they're drawn in the right world).
    pub fn move_peer(&mut self, id: u32, dim: Dim, to: Vec3) {
        let Some(mut peer) = self.remove_peer(id) else { return };
        if peer.dim != dim {
            self.forget_viewer(id);
        }
        peer.target = to;
        peer.pos = to;
        peer.dim = dim;
        self.enter(dim);
        self.peers.insert(id, peer);
        self.net_broadcast_all(crate::net::Msg::Dimension { id, dim: dim.index(), x: to.x, y: to.y, z: to.z });
        // They start that world afresh: everything changed in it so far.
        for m in self.world_msgs() {
            self.net_send_to(id, m);
        }
    }

    /// The host: keep the active dimension going for the joined players in it
    /// when the local player isn't there (or there is none).
    pub fn realm_tick(&mut self, dt: f32) {
        self.liquid_tick(dt);
        self.zap_tick(dt);
        self.vehicles_tick(dt, 0.0, 0.0, false);
        let centers: Vec<(Vec3, i32)> = self.peers.values().map(|p| (p.target, 4)).collect();
        self.world.stream(&centers);
        self.world.dirty.clear(); // nothing to draw (`move_local_player` redraws it all)
        if !self.peers.is_empty() {
            self.update_entities(dt);
        }
        // (Its sounds went out with `net_send`; nobody here to hear them.)
    }

    /// The host: every other dimension with joined players in it goes on.
    pub fn foreign_tick(&mut self, dt: f32) {
        if self.is_client() || self.menu {
            return;
        }
        for d in self.realm_dims() {
            if d == self.realm_dim() || self.parked.get(&d).is_none_or(|r| r.peers.is_empty()) {
                continue;
            }
            self.in_realm(d, |g| g.realm_tick(dt));
        }
    }

    /// Which dimension a joined player is in (None: no such player).
    pub fn peer_dim(&self, id: u32) -> Option<Dim> {
        if self.peers.contains_key(&id) {
            return Some(self.realm_dim());
        }
        self.parked.iter().find(|(_, r)| r.peers.contains_key(&id)).map(|(d, _)| *d)
    }

    /// Every joined player, in whichever dimension.
    pub fn all_peers(&self) -> impl Iterator<Item = (&u32, &Peer)> {
        self.peers.iter().chain(self.parked.values().flat_map(|r| r.peers.iter()))
    }

    /// A joined player, in whichever dimension.
    pub fn peer_ref(&self, id: u32) -> Option<&Peer> {
        self.peers.get(&id).or_else(|| self.parked.values().find_map(|r| r.peers.get(&id)))
    }

    pub fn peer_mut(&mut self, id: u32) -> Option<&mut Peer> {
        match self.peers.contains_key(&id) {
            true => self.peers.get_mut(&id),
            false => self.parked.values_mut().find_map(|r| r.peers.get_mut(&id)),
        }
    }

    /// Take a joined player out of whichever dimension they're in.
    pub fn remove_peer(&mut self, id: u32) -> Option<Peer> {
        self.peers.remove(&id).or_else(|| self.parked.values_mut().find_map(|r| r.peers.remove(&id)))
    }

    /// Joined players: someone else went to `dim`.
    pub fn client_peer_dimension(&mut self, id: u32, dim: Dim) {
        let Some(mut p) = self.remove_peer(id) else { return };
        p.dim = dim;
        self.in_realm(dim, |g| g.peers.insert(id, p));
    }

    /// Joined players: the host moved us to `to` in `dim`. Whatever we had of
    /// the worlds goes (the host sends what's there afresh); only who's where
    /// is kept.
    pub fn client_change_dimension(&mut self, dim: Dim, to: Vec3) {
        let mut everyone: Vec<(u32, Peer)> = std::mem::take(&mut self.peers).into_iter().collect();
        for (_, r) in self.parked.drain() {
            everyone.extend(r.peers);
        }
        let mut fresh = Realm::new(dim, &self.world, self.world.generator.opts);
        fresh.swap_with(self);
        self.move_local_player(dim, to);
        for (id, p) in everyone {
            let d = p.dim;
            self.in_realm(d, |g| g.peers.insert(id, p));
        }
        self.msg(format!("You're in {}.", dim.name()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    #[test]
    fn realms_swap_in_and_out_whole() {
        let mut g = crate::game::tests::arena(5);
        assert_eq!(g.realm_dim(), Dim::Over);
        g.world.set_v(ivec3(1, 50, 1), crate::block::GOLD_BLOCK);
        g.enter(Dim::Scorch);
        assert_eq!(g.realm_dim(), Dim::Scorch);
        assert!(g.away(), "the local player is still in the Overworld");
        assert!(g.mobs.is_empty());
        g.alloc_mob(crate::entity::MobKind::Grumbler, Vec3::new(3.0, 40.0, 3.0));
        g.enter(Dim::Over);
        assert!(!g.away());
        assert_eq!(g.world.get_v(ivec3(1, 50, 1)), crate::block::GOLD_BLOCK, "the Overworld came back as it was");
        assert!(g.mobs.iter().all(|m| m.kind != crate::entity::MobKind::Grumbler), "the Grumbler stayed in the Scorchlands");
        let n = g.in_realm(Dim::Scorch, |g| g.mobs.len());
        assert_eq!(n, 1);
        assert_eq!(g.realm_dims(), vec![Dim::Over, Dim::Scorch]);
    }
}
