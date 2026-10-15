//! Cosmetic meals: prey leaves simulation immediately; only a bounded visual remains.
use crate::{entity::{self, Mob, MobKind}, game::Game, render::{DynGeo, Pass}, sound::Sfx};
use macroquad::prelude::*;

pub const DURATION: f32 = 1.8;
pub const GULP: f32 = 1.05;
pub const CAP: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub struct Swallow {
    pub eater: u32,
    pub prey: u32,
    pub kind: u8,
    pub from: Vec3,
    pub yaw: f32,
    pub scale: f32,
    pub sheared: bool,
    pub elapsed: f32,
}
impl Swallow {
    pub fn meal(eater: u32, m: &Mob) -> Self {
        Self { eater, prey:m.id, kind:m.kind.index(), from:m.body.pos, yaw:m.yaw,
            scale:m.size * if m.baby>0.0 {0.55}else{1.0}, sheared:m.sheared, elapsed:0.0 }
    }
    pub fn valid(&self) -> bool {
        matches!(MobKind::from_index(self.kind),Some(MobKind::Mooer|MobKind::Oinker|MobKind::Fluffer|MobKind::Cluckster))
            && self.eater!=self.prey && self.from.is_finite() && self.yaw.is_finite()
            && self.scale.is_finite() && (0.1..=4.0).contains(&self.scale)
            && self.elapsed.is_finite() && (0.0..=DURATION).contains(&self.elapsed)
    }
    /// Ease into the mouth, with an exaggerated hop and a last-second squash.
    pub fn pose(&self, mouth: Vec3) -> (Vec3, Vec3) {
        let t=((self.elapsed-0.12)/(GULP-0.12)).clamp(0.0,1.0);
        let q=t*t*(3.0-2.0*t);
        let k=(1.0-q).max(0.0);
        let center=self.from+Vec3::Y*(MobKind::from_index(self.kind).map(|k|k.dims().1).unwrap_or(1.0)*self.scale*0.5);
        (center.lerp(mouth,q)+Vec3::Y*(std::f32::consts::PI*t).sin()*0.65,
            Vec3::new(k.powf(0.7),k.powf(1.3),k)*self.scale)
    }
}
/// Mouth and belly share a clock on host, observers, and late joiners.
pub fn opening(t:f32)->f32 { ((t/0.18).clamp(0.0,1.0)*((GULP+0.12-t)/0.22).clamp(0.0,1.0)).max(0.0) }
pub fn belly(t:f32)->Vec3 {
    let b=if t>=GULP {((t-GULP)/0.75*std::f32::consts::PI).sin().max(0.0)}else{0.0};
    Vec3::new(1.0+0.22*b,1.0-0.10*b,1.0+0.22*b)
}
pub fn mouth(m:&Mob)->Vec3 {
    m.body.pos+Mat3::from_rotation_y(-m.yaw)*Vec3::new(0.0,0.50,-0.57)*m.size
        -Vec3::Y*if m.sitting {0.15*m.size}else{0.0}
}
impl Game {
    pub(crate) fn swallows_tick(&mut self,dt:f32) {
        let host=!self.is_client();let mut burps=std::collections::BTreeMap::new();
        for f in &mut self.swallows {
            let before=f.elapsed;f.elapsed+=dt.max(0.0);
            if host && before<GULP && f.elapsed>=GULP
                && let Some(m)=self.mobs.iter().find(|m|m.id==f.eater && m.health>0.0) {
                burps.insert(f.eater,mouth(m));
            }
        }
        self.swallows.retain(|f|f.elapsed<DURATION);
        // One burp per eater even when an entire flock arrived together.
        for p in burps.into_values() {self.sfx(Sfx::Burp,Some(p));self.smoke(p,3,0.12);}
    }
    pub(crate) fn draw_swallows(&self,g:&mut DynGeo) {
        for f in &self.swallows {
            if f.elapsed>=GULP {continue;}
            let Some(m)=self.mobs.iter().find(|m|m.id==f.eater && m.health>0.0) else {continue};
            let Some(kind)=MobKind::from_index(f.kind) else {continue};
            let (pos,scale)=f.pose(mouth(m));
            let root=Mat4::from_translation(pos)*Mat4::from_rotation_y(-f.yaw+(f.elapsed*9.0).sin()*0.15)
                *Mat4::from_scale(scale)*Mat4::from_translation(-Vec3::Y*kind.dims().1*0.5);
            g.begin(Pass::Opaque,[1.0;4],false);
            let parts=if kind==MobKind::Fluffer && f.sheared {&entity::FLUFFER_SHEARED[..]}else{entity::model(kind)};
            entity::draw_model(g,&root,parts,0.0,self.world.shade_near(pos),false);
        }
    }
}
/// A dark open maw, ivory teeth and a little tongue, drawn over the face.
pub fn draw_mouth(g:&mut DynGeo,root:&Mat4,t:f32,sky:f32) {
    let open=opening(t);if open<=0.0 {return;}
    let face=*root*Mat4::from_rotation_y(std::f32::consts::PI);
    for (lo,size,color) in [
        (Vec3::new(-0.45,0.50-0.27*open,0.505),Vec3::new(0.90,0.54*open,0.018),[0.07,0.025,0.025,1.0]),
        (Vec3::new(-0.25,0.50+0.20*open,0.526),Vec3::new(0.50,0.055,0.025),[1.0,0.92,0.73,1.0]),
        (Vec3::new(-0.23,0.50-0.27*open,0.526),Vec3::new(0.46,0.045,0.025),[1.0,0.92,0.73,1.0]),
        (Vec3::new(-0.14,0.50-0.22*open,0.554),Vec3::new(0.28,0.045,0.06),[0.85,0.22,0.28,1.0]),
    ] {
        g.begin(Pass::Opaque,color,false);
        g.cube(&(face*Mat4::from_translation(lo)*Mat4::from_scale(size)),[crate::texture::T_WHITE;6],sky,[0.0,0.0,1.0,1.0]);
    }
}
