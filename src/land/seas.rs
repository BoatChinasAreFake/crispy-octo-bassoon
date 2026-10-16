//! Busier seas: kelp forests, seagrass meadows and glowing sea pickles, and
//! oceans that come warm, lukewarm, ordinary or frozen.
//!
//! - **Kelp** stands in tall columns on cool and ordinary sea floors and
//!   grows a little taller now and then, up to just under the surface.
//!   Break any of it and everything above comes away too. Dry it in a
//!   furnace for **Dried Kelp** (a crunchy snack).
//! - **Seagrass** carpets shallow floors; **sea pickles** glow in clusters
//!   on warm reefs.
//! - All three sit in water (they're waterlogged): you swim through them, and
//!   breaking one leaves water behind.
//! - **Warm oceans** are clear and full of coral; **lukewarm** ones have a
//!   little coral and plenty of seagrass; **frozen** ones are capped with ice
//!   and dotted with icebergs.

use crate::block::*;
use crate::game::Game;
use crate::noise::hash2;
use crate::world::{Biome, Generator};
use macroquad::math::{IVec3, Vec3};

/// The tallest kelp gets, and how often it grows.
pub const KELP_MAX: i32 = 16;
pub const KELP_CHANCE: f32 = 0.08;

/// What grows on this patch of sea floor, if anything: the plant, and (for
/// kelp) how tall. `depth` is how much water there is above the floor.
pub fn floor_plant(g: &Generator, biome: Biome, x: i32, z: i32, depth: i32) -> Option<(Id, i32)> {
    if g.opts.version >= 8 && biome.is_ocean() && biome!=Biome::FrozenOcean && (1..=6).contains(&depth) && hash2(g.seed^0xB3AC7,x,z)<0.012 {return Some((SHELL_PATCH_WET,1));}
    if depth < 2 || biome == Biome::FrozenOcean {
        return None;
    }
    let s = g.seed;
    let r = hash2(s ^ 0x5EA1, x, z);
    // Kelp forests in cool and ordinary seas, in patches.
    if matches!(biome, Biome::Ocean | Biome::LukewarmOcean) && depth >= 4 && hash2(s ^ 0x5EA2, x >> 3, z >> 3) < 0.4 && r < 0.18 {
        let tall = (2 + (hash2(s ^ 0x5EA3, x, z) * (depth as f32 - 1.0)) as i32).min(depth - 1).min(KELP_MAX);
        return Some((KELP, tall));
    }
    // Sea pickles on warm floors.
    if biome == Biome::WarmOcean && r < 0.025 {
        return Some((SEA_PICKLE, 1));
    }
    // Seagrass in shallow-ish water, thicker in warmer seas.
    let grass = match biome {
        Biome::WarmOcean | Biome::LukewarmOcean => 0.3,
        _ => 0.12,
    };
    (depth <= 14 && r < grass + 0.18 && r >= 0.18).then_some((SEAGRASS, 1))
}

impl Generator {
    /// Small, sheltered basins in level beach sand. The complete rim is
    /// checked from world coordinates, so neighbouring chunks agree.
    pub(crate) fn beach_pool_cell(&self,cx:i32,cz:i32)->Option<(i32,i32,i32,i32)> {
        if self.opts.version < 8 {return None;}
        let s=self.seed^0x71DE;
        if hash2(s,cx,cz)>0.28 {return None;}
        let (x,z)=(cx*64+8+(hash2(s^1,cx,cz)*48.0) as i32,cz*64+8+(hash2(s^2,cx,cz)*48.0) as i32);
        let (h,b)=self.column(x,z);
        let min_height=self.sea();
        if b!=Biome::Beach || !(min_height..=self.sea()+3).contains(&h) || self.climate(x,z).0< -0.1 {return None;}
        let radius=2+(hash2(s^3,cx,cz)*2.0) as i32;
        for dz in -radius-1..=radius+1 {for dx in -radius-1..=radius+1 {
            let (hh,bb)=self.column(x+dx,z+dz);
            if bb!=Biome::Beach || hh<h || hh>h+1 || self.mod_biome_at(x+dx,z+dz,bb).is_some() {return None;}
        }}
        Some((x,z,h-1,radius))
    }

    pub(crate) fn beach_pool_at(&self,x:i32,z:i32)->Option<i32> {
        if self.opts.version < 8 {return None;}
        let (cx,cz)=(x.div_euclid(64),z.div_euclid(64));
        let s=self.seed^0x71DE;
        let (mx,mz)=(cx*64+8+(hash2(s^1,cx,cz)*48.0) as i32,cz*64+8+(hash2(s^2,cx,cz)*48.0) as i32);
        if (x-mx).pow(2)+(z-mz).pow(2)>9 {return None;}
        let (_,_,level,radius)=self.beach_pool_cell(cx,cz)?;
        ((x-mx).pow(2)+(z-mz).pow(2)<=radius*radius).then_some(level)
    }


    /// Beach scrub, shells, dune grass, the odd run of driftwood and sparse turtle nests.
    /// The shared cell anchor makes logs continuous across chunk boundaries.
    pub fn beach_feature(&self,x:i32,z:i32,h:i32)->Option<Id> {
        if self.opts.version < 8 || h<self.sea()+1 { return None; }
        let s=self.seed^0xB3AC0;
        let (cx,cz)=(x.div_euclid(32),z.div_euclid(32));
        if hash2(s,cx,cz)<0.12 {
            let (rx,rz)=(cx*32+8+(hash2(s^1,cx,cz)*16.0) as i32,cz*32+8+(hash2(s^2,cx,cz)*16.0) as i32);
            let len=2+(hash2(s^3,cx,cz)*3.0) as i32;
            let along=if hash2(s^4,cx,cz)<0.5 { z==rz && (rx..rx+len).contains(&x) } else { x==rx && (rz..rz+len).contains(&z) };
            if along && self.column(rx,rz)==(h,Biome::Beach) {return Some(if hash2(s^4,cx,cz)<0.5 {DRIFTWOOD_Z}else{DRIFTWOOD});}
        }
        let r=hash2(s^5,x,z);
        if r>0.006 && r<0.065 && h<=self.sea()+2 {return Some(SHELL_PATCH);}
        if (0.006..0.12).contains(&r) && h>=self.sea()+2 && hash2(s^6,x>>3,z>>3)<0.6 {return Some(DUNE_GRASS);}
        if r<0.0003 && self.climate(x,z).0>0.1 { Some(TURTLE_EGG) }
        else if r<0.006 { Some(DEAD_BUSH) }
        else if r<0.045 && hash2(s^6,x>>3,z>>3)<0.35 { Some(TALL_GRASS) }
        else { None }
    }

    /// How far an iceberg rises above a frozen sea here (0: none). Bergs sit
    /// on a grid, one per 24 blocks or so, each a lumpy mound of packed ice.
    pub fn iceberg(&self, x: i32, z: i32) -> i32 {
        const GRID: i32 = 24;
        let s = self.seed ^ 0x1CEB;
        let (gx, gz) = (x.div_euclid(GRID), z.div_euclid(GRID));
        let mut best = 0;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let (cx, cz) = (gx + dx, gz + dz);
                if hash2(s, cx, cz) > 0.35 {
                    continue;
                }
                let mid = (cx * GRID + 4 + (hash2(s ^ 1, cx, cz) * 16.0) as i32, cz * GRID + 4 + (hash2(s ^ 2, cx, cz) * 16.0) as i32);
                let d = (((x - mid.0).pow(2) + (z - mid.1).pow(2)) as f32).sqrt();
                let radius = 3.0 + hash2(s ^ 3, cx, cz) * 4.0;
                let height = 3.0 + hash2(s ^ 4, cx, cz) * 9.0;
                let here = height * (1.0 - (d / radius).powi(2));
                if here >= 1.0 {
                    best = best.max(here as i32);
                }
            }
        }
        best
    }
}

impl Game {
    /// Check the entire collision box, including Tuna's wider body.
    pub(crate) fn coastal_room(&self,kind:crate::entity::MobKind,at:Vec3)->bool {
        let (half,height)=kind.dims();
        let lo=at-Vec3::new(half,0.0,half);
        let hi=at+Vec3::new(half,height,half);
        for z in lo.z.floor() as i32..=(hi.z-0.001).floor() as i32 {
            for x in lo.x.floor() as i32..=(hi.x-0.001).floor() as i32 {
                if !self.world.is_loaded(x,z) { return false; }
                for y in lo.y.floor() as i32..=(hi.y-0.001).floor() as i32 {
                    let id=self.world.get(x,y,z);
                    if kind==crate::entity::MobKind::Crab {
                        if is_solid(id) || is_lava(id) { return false; }
                    } else if !is_water(id) && !waterlogged(id) { return false; }
                }
            }
        }
        true
    }

    /// Species-specific habitat using actual loaded blocks, rather than just
    /// generated height (so player-built dry/filled pools cannot spawn fish).
    pub(crate) fn coastal_habitat(&self,kind:crate::entity::MobKind,x:i32,z:i32)->Option<Vec3> {
        use crate::entity::MobKind::*;
        let g=&self.world.generator;
        if !self.world.is_loaded(x,z) { return None; }
        let (_,biome)=g.column(x,z);
        if !biome.is_ocean() && biome!=Biome::Beach { return None; }
        if biome==Biome::FrozenOcean { return None; }
        let sea=self.world.sea();
        let floor=(4..=sea+4).rev().find(|&y|is_solid(self.world.get(x,y,z)))?;
        let ground=self.world.get(x,floor,z);
        let water_top=(floor+1..=sea+4).filter(|&y|is_wet(self.world.get(x,y,z))).max().unwrap_or(sea);
        let depth=water_top-floor;
        let sandy=matches!(ground,SAND|GRAVEL|RED_SAND);
        let rocky=matches!(ground,STONE|GRAVEL|DEEPSLATE);
        let fits=match kind {
            Crab => (biome==Biome::Beach || biome.is_ocean()) && sandy && (-4..=4).contains(&depth),
            Starfish => (1..=8).contains(&depth) && sandy,
            Clam => (2..=10).contains(&depth) && sandy,
            Mussel | Oyster => (1..=8).contains(&depth) && (sandy||rocky),
            SeaSnail => (1..=8).contains(&depth) && (sandy||rocky),
            Sardine => biome.is_ocean() && depth>=5,
            Tuna => biome.is_ocean() && depth>=12,
            Eel => biome.is_ocean() && (4..=24).contains(&depth),
            _=>false,
        };
        if !fits { return None; }
        let y=if kind.benthic() {floor+1} else {match kind { Sardine=>water_top-3, Tuna=>water_top-6, _=>floor+2 }};
        let at=Vec3::new(x as f32+0.5,y as f32,z as f32+0.5);
        self.coastal_room(kind,at).then_some(at)
    }

    pub(crate) fn try_coastal_wildlife(&mut self,center:Vec3)->bool {
        use crate::entity::MobKind::*;
        let mut centers=vec![center];
        if !self.away() { centers.push(self.player.body.pos); }
        centers.extend(self.peers.values().map(|p|p.target));
        self.mobs.retain(|m|m.persistent || !m.kind.coastal() || centers.iter().any(|p|m.body.pos.distance_squared(*p)<=192.0*192.0));
        let cap=(80+12*self.peers.len()).min(160);
        let count=self.mobs.iter().filter(|m|m.kind.coastal()).count();
        if count>=cap { return false; }
        for _ in 0..4 {
            let angle=self.rng.range(0.0,std::f32::consts::TAU);
            let radius=self.rng.range(18.0,50.0);
            let (x,z)=((center.x+angle.cos()*radius).floor() as i32,(center.z+angle.sin()*radius).floor() as i32);
            let kinds=[Starfish,Clam,Crab,Sardine,Tuna,Mussel,Oyster,SeaSnail,Eel];
            let kind=kinds[self.rng.int(0,8) as usize];
            let limit=match kind { Sardine=>20, Tuna=>4, Eel=>6, Crab=>8, _=>6 };
            let nearby=self.mobs.iter().filter(|m|m.kind==kind && m.body.pos.distance_squared(center)<96.0*96.0).count();
            if nearby>=limit { continue; }
            if self.coastal_habitat(kind,x,z).is_none() { continue; }
            let n=if kind==Sardine {self.rng.int(5,8) as usize}else if matches!(kind,Mussel|Oyster|Clam) {self.rng.int(2,3) as usize}else{1};
            let school=self.rng.int(0,1_000_000) as u32;
            let mut spawned=false;
            for i in 0..n.min(limit-nearby).min(cap-count) {
                let (xx,zz)=(x+i as i32,z);
                // Recheck each member's own floor/habitat: no hovering colonies.
                let Some(pos)=self.coastal_habitat(kind,xx,zz) else { continue; };
                if self.mobs.iter().any(|m|m.kind.coastal() && m.body.pos.distance_squared(pos)<0.7*0.7) { continue; }
                let id=self.alloc_mob(kind,pos);
                if let Some(m)=self.mobs.iter_mut().find(|m|m.id==id) {m.seed=school;m.yaw=angle;m.wander_dir=Some(angle);m.body.in_water=is_water(self.world.get_v(pos.floor().as_ivec3())) || waterlogged(self.world.get_v(pos.floor().as_ivec3()));}
                spawned=true;
            }
            if spawned { return true; }
        }
        false
    }


    pub(crate) fn sea_spawn_room(&self,at:Vec3)->bool {
        let wet=|id|is_water(id)||waterlogged(id);
        [Vec3::ZERO,Vec3::new(0.5,0.0,0.0),Vec3::new(-0.5,0.0,0.0),Vec3::new(0.0,0.6,0.0),Vec3::new(0.0,0.0,0.5),Vec3::new(0.0,0.0,-0.5)]
            .iter().all(|&off|wet(self.world.get_v((at+off).floor().as_ivec3())))
    }

    /// A dedicated marine pass: local schools/pods don't share land-animal
    /// caps, while hard world caps bound memory and host simulation work.
    pub(crate) fn try_sea_wildlife(&mut self,center:Vec3)->bool {
        use crate::entity::MobKind;
        let mut centers=vec![center];
        if !self.away() { centers.push(self.player.body.pos); }
        centers.extend(self.peers.values().map(|p|p.target));
        // Recycle wild schools outside every player's loaded surroundings;
        // kept animals remain, and still count against the hard limit.
        self.mobs.retain(|m|m.persistent || !matches!(m.kind,MobKind::Fishy|MobKind::Dolphin)
            || centers.iter().any(|p|m.body.pos.distance_squared(*p)<=192.0*192.0));
        let fish_cap=(64+8*self.peers.len()).min(128);
        let dolphin_cap=(12+2*self.peers.len()).min(24);
        for _ in 0..4 {
            let angle=self.rng.range(0.0,std::f32::consts::TAU);
            let radius=self.rng.range(20.0,52.0);
            let (x,z)=((center.x+angle.cos()*radius).floor() as i32,(center.z+angle.sin()*radius).floor() as i32);
            if !self.world.is_loaded(x,z) { continue; }
            let biome=self.world.generator.column(x,z).1;
            if !biome.is_ocean() { continue; }
            let sea=self.world.sea();
            let wet=|id| is_water(id)||waterlogged(id);
            let top=if biome==Biome::FrozenOcean { sea-1 } else { sea };
            let depth=(0..top).take_while(|d| wet(self.world.get(x,top-d,z))).count() as i32;
            if depth<3 { continue; }
            let fish=self.mobs.iter().filter(|m|m.kind==MobKind::Fishy).count();
            let dolphins=self.mobs.iter().filter(|m|m.kind==MobKind::Dolphin).count();
            let local_fish=self.mobs.iter().filter(|m|m.kind==MobKind::Fishy && m.body.pos.distance_squared(center)<96.0*96.0).count();
            let local_dolphins=self.mobs.iter().filter(|m|m.kind==MobKind::Dolphin && m.body.pos.distance_squared(center)<96.0*96.0).count();
            let (kind,n,y)=if biome!=Biome::FrozenOcean && depth>=5 && !self.is_night() && dolphins<dolphin_cap && local_dolphins<6 && self.rng.chance(0.22) {
                (MobKind::Dolphin,(self.rng.int(1,2) as usize).min(dolphin_cap-dolphins).min(6-local_dolphins),sea-2)
            } else if fish<fish_cap && local_fish<24 && self.rng.chance(0.8) {
                (MobKind::Fishy,(self.rng.int(4,7) as usize).min(fish_cap-fish).min(24-local_fish),top-1-self.rng.int(0,(depth-3).min(10)))
            } else { continue; };
            let mut spawned=false;
            for i in 0..n {
                let at=Vec3::new(x as f32+0.5+i as f32*0.8,y as f32+0.5,z as f32+0.5-i as f32*0.6);
                if self.sea_spawn_room(at) { self.alloc_mob(kind,at);spawned=true; }
            }
            if spawned { return true; }
        }
        false
    }

    /// A waterlogged block was broken (where the world lives): the water
    /// stays, and broken kelp takes the kelp above it with it.
    pub fn sea_block_gone(&mut self, p: IVec3, old: Id) {
        if !waterlogged(old) {
            return;
        }
        self.refill.push(p);
        if old == KELP {
            let mut q = p + IVec3::Y;
            while self.world.get_v(q) == KELP {
                self.world.set_v(q, WATER);
                self.pop_drop(q.as_vec3() + Vec3::splat(0.5), KELP, 1);
                q += IVec3::Y;
            }
            if !self.away() && p.as_vec3().distance(self.player.body.pos) < 8.0 {
                self.advance("kelp_me");
            }
        }
    }

    /// Water back into broken waterlogged blocks (a tick later, once they're gone).
    pub fn refill_tick(&mut self) {
        for p in std::mem::take(&mut self.refill) {
            // (Running water may have got there first.)
            let here = self.world.get_v(p);
            if here == AIR || (is_water(here) && here != WATER) {
                self.world.set_v(p, WATER);
            }
        }
    }

    /// Kelp grows: a random tick on the top of a column.
    pub fn kelp_grows(&mut self, p: IVec3) {
        let up = p + IVec3::Y;
        // (Only into water, and always leaving a block of it on top.)
        if self.world.get_v(up) != WATER || !is_water(self.world.get_v(up + IVec3::Y)) || !self.rng.chance(KELP_CHANCE) {
            return;
        }
        let mut height = 1;
        while height < KELP_MAX && self.world.get_v(p - IVec3::Y * height) == KELP {
            height += 1;
        }
        if height < KELP_MAX {
            self.world.set_v(up, KELP);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use macroquad::math::ivec3;

    #[test]
    fn tide_pools_are_supported_contained_and_cross_chunk_safe() {
        let _lock=crate::mods::registry_test_lock();
        for seed in [77,1234] {
            let opts=crate::world::GenOptions{structures:0,..crate::world::GenOptions::DEFAULT};
            let mut g=Game::new_with(seed,true,false,opts);
            let beach_generator=&g.world.generator;
            let found=(-128..128).flat_map(|z|(-128..128).map(move|x|(x,z))).filter_map(|(x,z)|beach_generator.beach_pool_cell(x,z))
                .find(|&(x,z,_,r)|x.rem_euclid(16)<r || x.rem_euclid(16)>=16-r || z.rem_euclid(16)<r || z.rem_euclid(16)>=16-r).expect("pool across a chunk edge");
            let (x,z,level,radius)=found;
            for dz in -1..=1 {for dx in -1..=1 {g.world.load_now(x.div_euclid(16)+dx,z.div_euclid(16)+dz);}}
            assert_eq!(g.world.get(x,level-2,z),SAND);
            assert_eq!(g.world.get(x,level-3,z),SANDSTONE);
            assert!(is_wet(g.world.get(x,level,z)));
            assert!(g.coastal_habitat(crate::entity::MobKind::Clam,x,z).is_some());
            assert!(g.coastal_habitat(crate::entity::MobKind::Tuna,x,z).is_none());
            for dz in -radius..=radius {for dx in -radius..=radius {
                if dx*dx+dz*dz<=radius*radius {assert!(is_wet(g.world.get(x+dx,level,z+dz)));}
            }}
            for (dx,dz) in [(radius+1,0),(-radius-1,0),(0,radius+1),(0,-radius-1)] {assert!(is_solid(g.world.get(x+dx,level,z+dz)),"closed pool rim");}
            for _ in 0..100 {g.liquid_tick(0.05);g.falling_tick(0.05);}
            assert!(is_wet(g.world.get(x,level,z)),"pool retained its water");
            assert_eq!(g.world.get(x,level-2,z),SAND,"sand did not fall into caves");
            g.ready=true;g.player.body.pos=Vec3::new(x as f32+0.5,level as f32,z as f32+0.5);
            g.update(0.6,&crate::game::Controls::default());
            assert!(g.advancements.earned.iter().any(|a|a=="tide_time"));
        }
    }

    #[test]
    fn revision_eleven_beaches_have_collectible_finds_and_grass() {
        let _lock=crate::mods::registry_test_lock();
        let g=Generator::with(1234,crate::world::GenOptions{structures:0,..crate::world::GenOptions::DEFAULT});
        let mut found=std::collections::HashSet::new();
        'search: for z in (-4096..4096).step_by(8) {for x in (-4096..4096).step_by(8) {
            if g.column(x,z).1!=Biome::Beach {continue;}
            for dz in 0..8 {for dx in 0..8 {
                let (xx,zz)=(x+dx,z+dz);let (h,b)=g.column(xx,zz);
                if b!=Biome::Beach || g.beach_pool_at(xx,zz).is_some() {continue;}
                if let Some(id)=g.beach_feature(xx,zz,h) {
                    if !matches!(id,DRIFTWOOD|DRIFTWOOD_Z|SHELL_PATCH|DUNE_GRASS) || found.contains(&id) {continue;}
                    let blocks=g.generate(xx.div_euclid(16),zz.div_euclid(16));
                    if blocks[crate::world::idx(xx.rem_euclid(16),h+1,zz.rem_euclid(16))]==id {found.insert(id);}
                }
            }}
            if found.len()==4 {break 'search;}
        }}
        assert_eq!(found.len(),4,"{found:?}");
        assert!(crate::block::reg().recipes.iter().any(|r|r.inputs==[(DRIFTWOOD,1)] && r.output==(PLANKS,2)));
        assert_eq!(crate::containers::fuel_secs(DRIFTWOOD),Some(10.0));
        assert_eq!(block(SHELL_PATCH).drop,SEASHELL);
    }

    #[test]
    fn coastal_habitats_respect_depth_support_and_body_clearance() {
        use crate::entity::MobKind::*;
        let _lock=crate::mods::registry_test_lock();
        let mut g=Game::new_with(77,true,false,crate::world::GenOptions::DEFAULT);
        let (x,z)=(-4096..4096).step_by(64).flat_map(|z|(-4096..4096).step_by(64).map(move|x|(x,z)))
            .find(|&(x,z)|g.world.generator.column(x,z).1==Biome::Ocean).unwrap();
        for dz in -1..=1 { for dx in -1..=1 {g.world.load_now(x.div_euclid(16)+dx,z.div_euclid(16)+dz);} }
        let sea=g.world.sea();
        for depth in [2,6,16] {
            for zz in z-3..=z+3 {for xx in x-3..=x+3 {for y in 4..=sea+4 {
                g.world.set(xx,y,zz,if y<=sea-depth {SAND}else if y<=sea {WATER}else{AIR});
            } } }
            for kind in [Starfish,Clam,Crab,Sardine,Tuna,Mussel,Oyster,SeaSnail,Eel] {
                let expected=match kind {Crab=>depth<=4,Starfish|Clam|Mussel|Oyster|SeaSnail=>depth<=8,Sardine=>depth>=5,Tuna=>depth>=12,Eel=>depth>=4,_=>false};
                let at=g.coastal_habitat(kind,x,z);
                assert_eq!(at.is_some(),expected,"{kind:?} at depth {depth}");
                if let Some(at)=at {assert!(g.coastal_room(kind,at));}
            }
        }
        let at=g.coastal_habitat(Tuna,x,z).unwrap();
        g.world.set(x+1,at.y as i32,z,STONE);
        assert!(!g.coastal_room(Tuna,at),"a Tuna needs more than a point of water");
        g.world.set(x+1,at.y as i32,z,WATER);
        g.world.set(x,at.y as i32,z,AIR);
        assert!(!g.coastal_room(Tuna,at),"dry cells reject fish");
    }

    #[test]
    fn coastal_spawning_has_local_and_global_limits_and_recycles_wildlife() {
        use crate::entity::MobKind::*;
        let _lock=crate::mods::registry_test_lock();
        let mut g=Game::new_with(77,true,false,crate::world::GenOptions::DEFAULT);
        let (x,z)=(-4096..4096).step_by(64).flat_map(|z|(-4096..4096).step_by(64).map(move|x|(x,z)))
            .find(|&(x,z)|[-64,0,64].iter().all(|&dx|[-64,0,64].iter().all(|&dz|{
                let (h,b)=g.world.generator.column(x+dx,z+dz);b==Biome::Ocean && h<g.world.sea()-12
            }))).unwrap();
        for dz in -4..=4 {for dx in -4..=4 {g.world.load_now(x.div_euclid(16)+dx,z.div_euclid(16)+dz);} }
        let center=Vec3::new(x as f32+0.5,g.world.sea() as f32,z as f32+0.5);
        for _ in 0..200 {g.try_coastal_wildlife(center);}
        for kind in [Sardine,Tuna] {
            let n=g.mobs.iter().filter(|m|m.kind==kind).count();
            assert!(n>0 && n<=if kind==Sardine {20}else{4},"{kind:?}: {n}");
        }
        assert!(g.mobs.iter().filter(|m|m.kind.coastal()).all(|m|g.coastal_room(m.kind,m.body.pos)));
        g.mobs.clear();
        for _ in 0..80 {let id=g.alloc_mob(Sardine,center+Vec3::X*10000.0);g.mobs.iter_mut().find(|m|m.id==id).unwrap().persistent=true;}
        assert!(!g.try_coastal_wildlife(center));
        assert_eq!(g.mobs.len(),80,"kept animals respect hard limits");
        for m in &mut g.mobs {m.persistent=false;}
        for _ in 0..100 {g.try_coastal_wildlife(center);}
        assert!(g.mobs.iter().all(|m|m.body.pos.distance_squared(center)<96.0*96.0));
        assert!(g.mobs.iter().any(|m|m.kind==Sardine));
    }

    #[test]
    fn bottom_creatures_stay_on_the_floor_and_shellfish_do_not_wander() {
        use crate::entity::{Mob,MobKind};
        let _lock=crate::mods::registry_test_lock();
        let mut g=Game::new_with(9,true,true,crate::world::GenOptions::DEFAULT);
        g.world.load_now(0,0);
        for z in 0..16 {for x in 0..16 {for y in 45..60 {g.world.set(x,y,z,if y<=49 {SAND}else{WATER});}}}
        let mut rng=crate::noise::Rng::new(8);
        for kind in [MobKind::Clam,MobKind::Mussel,MobKind::Oyster,MobKind::Starfish,MobKind::SeaSnail,MobKind::Crab,MobKind::Eel] {
            let mut m=Mob::new(kind,Vec3::new(8.5,50.0,8.5),&mut rng);m.body.in_water=true;
            for _ in 0..300 {m.update(1.0/30.0,&g.world,Vec3::splat(100.0),false,1.0,&mut rng);}
            assert!(m.body.pos.y<51.2 && m.body.pos.y>=49.9,"{kind:?} floated or sank: {}",m.body.pos.y);
            assert_eq!(m.health,kind.max_health());
            if matches!(kind,MobKind::Clam|MobKind::Mussel|MobKind::Oyster) {assert!((m.body.pos-Vec3::new(8.5,50.0,8.5)).length()<0.02);}
        }
        g.menu=false;
        g.item_advancements(RAW_SARDINE);
        assert!(g.advancements.earned.iter().any(|a|a=="shore_supper"));
    }

    #[test]
    fn new_species_save_and_food_paths_round_trip() {
        use crate::entity::{Mob,MobKind};
        let _lock=crate::mods::registry_test_lock();
        let mut rng=crate::noise::Rng::new(4);
        let mut mobs=Vec::new();
        for kind in MobKind::ALL.into_iter().filter(|k|k.coastal()) {
            assert!(!kind.hostile() && !kind.passive());
            assert_eq!(MobKind::from_name(kind.name()),Some(kind));
            let mut m=Mob::new(kind,Vec3::new(1.5,50.0,1.5),&mut rng);m.persistent=true;
            assert!(!crate::entity::model(kind).is_empty());
            if kind!=MobKind::Starfish {
                let (food,_)=m.loot(&mut rng).expect("harvestable seafood");
                assert_eq!(crate::containers::smelt(food),Some(COOKED_SEAFOOD));
                assert!(food_value(food).is_some());
            }
            mobs.push(m);
        }
        let bytes=crate::animals::encode_mobs(&mobs,&std::collections::HashMap::new());
        let back=crate::animals::decode_mobs(&bytes,&mut rng);
        assert_eq!(back.iter().map(|m|m.kind).collect::<Vec<_>>(),mobs.iter().map(|m|m.kind).collect::<Vec<_>>());
        assert_eq!(MobKind::SnowGolem.index(),67,"old base mob indices stay fixed");
        assert_eq!(reg().items[(SEASHELL-FIRST_ITEM) as usize].key,"seashell");
    }

    #[test]
    fn ocean_wildlife_forms_schools_without_land_caps_or_dry_spawns() {
        use crate::entity::MobKind;
        let _lock=crate::mods::registry_test_lock();
        let mut g=Game::new_with(77,true,false,crate::world::GenOptions::DEFAULT);
        let (x,z)=(-4096..4096).step_by(64).flat_map(|z|(-4096..4096).step_by(64).map(move|x|(x,z)))
            .find(|&(x,z)|[-64,0,64].iter().all(|&dx|[-64,0,64].iter().all(|&dz| {
                let (h,b)=g.world.generator.column(x+dx,z+dz);b==Biome::Ocean && h<g.world.sea()-12
            }))).expect("open ocean habitat");
        let center=Vec3::new(x as f32+0.5,g.world.sea() as f32+2.0,z as f32+0.5);
        for dz in -4..=4 { for dx in -4..=4 { g.world.load_now(x.div_euclid(16)+dx,z.div_euclid(16)+dz); } }
        for _ in 0..16 { g.alloc_mob(MobKind::Mooer,center+Vec3::Y*4.0); }
        for _ in 0..160 { g.try_sea_wildlife(center); }
        let fish=g.mobs.iter().filter(|m|m.kind==MobKind::Fishy).count();
        let dolphins=g.mobs.iter().filter(|m|m.kind==MobKind::Dolphin).count();
        assert!((18..=24).contains(&fish) && (2..=6).contains(&dolphins),"{fish} fish, {dolphins} dolphins");
        assert!(g.mobs.iter().filter(|m|matches!(m.kind,MobKind::Fishy|MobKind::Dolphin)).all(|m|g.sea_spawn_room(m.body.pos)));
        assert!(!g.sea_spawn_room(center+Vec3::Y*10.0));
        for m in &mut g.mobs { if matches!(m.kind,MobKind::Fishy|MobKind::Dolphin) { m.body.pos.x+=10000.0; } }
        for _ in fish..64 { g.alloc_mob(MobKind::Fishy,center+Vec3::X*10000.0); }
        for _ in dolphins..12 { g.alloc_mob(MobKind::Dolphin,center+Vec3::X*10000.0); }
        for m in &mut g.mobs { if matches!(m.kind,MobKind::Fishy|MobKind::Dolphin) { m.persistent=true; } }
        let count=g.mobs.len();
        for _ in 0..40 { assert!(!g.try_sea_wildlife(center)); }
        assert_eq!(g.mobs.len(),count,"distant schools still respect world caps");
        for m in &mut g.mobs { if matches!(m.kind,MobKind::Fishy|MobKind::Dolphin) { m.persistent=false; } }
        for _ in 0..100 { g.try_sea_wildlife(center); }
        assert!(g.mobs.iter().filter(|m|matches!(m.kind,MobKind::Fishy|MobKind::Dolphin)).all(|m|m.body.pos.distance_squared(center)<96.0*96.0));
        assert!(g.mobs.iter().filter(|m|m.kind==MobKind::Fishy).count()>=18,"wild slots are recycled at a new coast");
    }

    #[test]
    fn beach_details_use_supported_existing_blocks() {
        let _lock=crate::mods::registry_test_lock();
        let g=Generator::with(1234,crate::world::GenOptions{structures:0,..crate::world::GenOptions::DEFAULT});
        let mut found=std::collections::HashSet::new();
        let mut checked=0;
        'search: for z in (-4096..4096).step_by(8) { for x in (-4096..4096).step_by(8) {
            let (h,b)=g.column(x,z);
            if b!=Biome::Beach || h<g.sea()+1 { continue; }
            for dz in 0..8 { for dx in 0..8 {
                let (xx,zz)=(x+dx,z+dz);let (hh,bb)=g.column(xx,zz);
                if bb!=Biome::Beach { continue; }
                if let Some(id)=g.beach_feature(xx,zz,hh) {
                    if id==TURTLE_EGG { assert!(g.climate(xx,zz).0>0.1); }
                    assert!(hh>g.sea());found.insert(id);
                    if checked<12 {
                        let blocks=g.generate(xx.div_euclid(16),zz.div_euclid(16));
                        let ground=blocks[crate::world::idx(xx.rem_euclid(16),hh,zz.rem_euclid(16))];
                        assert!(matches!(ground,SAND|GRAVEL));checked+=1;
                    }
                }
            } }
            if found.len()>=5 && checked>=12 { break 'search; }
        } }
        assert!((found.contains(&DRIFTWOOD) || found.contains(&DRIFTWOOD_Z)) && found.contains(&DUNE_GRASS) && found.contains(&DEAD_BUSH) && found.contains(&TURTLE_EGG),"{found:?}");
    }

    /// A pool, 8 deep, over the arena's floor.
    fn pool(g: &mut Game) {
        for y in 50..58 {
            for z in -3..=3 {
                for x in -3..=3 {
                    g.world.set_v(ivec3(x, y, z), WATER);
                }
            }
        }
    }

    #[test]
    fn kelp_grows_and_comes_away_whole_leaving_water() {
        let mut g = crate::game::tests::arena(701);
        pool(&mut g);
        for y in 50..53 {
            g.world.set_v(ivec3(0, y, 0), KELP);
        }
        for _ in 0..400 {
            g.kelp_grows(ivec3(0, (50..58).rev().find(|&y| g.world.get(0, y, 0) == KELP).unwrap(), 0));
        }
        let top = (50..58).rev().find(|&y| g.world.get(0, y, 0) == KELP).unwrap();
        assert_eq!(top, 56, "it grows to just under the surface");
        // Break the bottom: it all comes away, the water stays, and each piece drops.
        g.break_block(ivec3(0, 50, 0), true);
        g.refill_tick();
        assert!((50..58).all(|y| g.world.get(0, y, 0) == WATER), "water where the kelp was");
        assert_eq!(g.drops.iter().filter(|d| d.item == KELP).map(|d| d.n as u32).sum::<u32>(), 7);
        // Dried in a furnace, it's a snack.
        assert_eq!(crate::containers::smelt(KELP), Some(DRIED_KELP));
        assert!(food_value(DRIED_KELP).is_some());
    }

    #[test]
    fn you_swim_through_sea_plants() {
        let mut g = crate::game::tests::arena(702);
        pool(&mut g);
        g.world.set_v(ivec3(1, 50, 1), SEAGRASS);
        g.world.set_v(ivec3(1, 51, 1), SEAGRASS);
        g.player.body.pos = Vec3::new(1.5, 50.1, 1.5);
        crate::entity::move_body(&g.world, &mut g.player.body, 0.05, false);
        assert!(g.player.body.in_water, "seagrass is wet");
        assert!(is_wet(SEA_PICKLE) && !is_wet(STONE));
    }

    #[test]
    fn running_water_leaves_sea_plants_be_and_they_feed_it() {
        let p = ivec3(0, 50, 0);
        // A seagrass under water, with air beside it: it stays put (it isn't washed out).
        let get = |q: IVec3| if q == p { SEAGRASS } else if q.y > p.y { WATER } else if q.y < p.y { STONE } else { AIR };
        assert_eq!(crate::liquids::settle(&get, p), None);
        // A gap between two kelp on the floor fills with a new source, as between two waters.
        let get = |q: IVec3| if q.y < p.y { STONE } else if q == p { AIR } else if q.y == p.y && (q.x - p.x).abs() == 1 && q.z == p.z { KELP } else { AIR };
        assert_eq!(crate::liquids::settle(&get, p), Some(WATER));
    }

    #[test]
    fn seas_come_warm_lukewarm_ordinary_and_frozen_with_their_own_floors() {
        let g = Generator::new(2024);
        let mut seen = std::collections::HashSet::new();
        let mut plants = std::collections::HashSet::new();
        for z in (-4000..4000).step_by(32) {
            for x in (-4000..4000).step_by(32) {
                let (h, b) = g.column(x, z);
                if b.is_ocean() {
                    seen.insert(format!("{b:?}"));
                    if let Some((p, _)) = floor_plant(&g, b, x, z, g.sea() - h) {
                        plants.insert(p);
                    }
                }
            }
        }
        for b in ["Ocean", "WarmOcean", "LukewarmOcean", "FrozenOcean"] {
            assert!(seen.contains(b), "no {b}");
        }
        assert!(plants.contains(&KELP) && plants.contains(&SEAGRASS) && plants.contains(&SEA_PICKLE));
    }
}
