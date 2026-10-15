//! Cosmetic meals: prey leaves simulation immediately; only a bounded visual remains.
use crate::{entity::{self, Mob, MobKind}, game::Game, render::{DynGeo, Pass}, sound::Sfx};
use macroquad::prelude::*;

pub const DURATION: f32 = 2.2;
pub const INHALE:f32=0.45;
pub const GULP: f32 = 1.45;
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
        let t=((self.elapsed-INHALE)/(GULP-INHALE)).clamp(0.0,1.0);
        let q=t*t*(3.0-2.0*t);
        let k=(1.0-q).max(0.0);
        let center=self.from+Vec3::Y*(MobKind::from_index(self.kind).map(|k|k.dims().1).unwrap_or(1.0)*self.scale*0.5);
        (center.lerp(mouth,q)+Vec3::Y*(std::f32::consts::PI*t).sin()*0.65,
            Vec3::new(k.powf(0.7),k.powf(1.3),k)*self.scale)
    }
}
/// Mouth and belly share a clock on host, observers, and late joiners.
pub fn opening(t:f32)->f32 { (((t-0.12)/0.33).clamp(0.0,1.0)*((GULP+0.12-t)/0.22).clamp(0.0,1.0)).max(0.0) }
pub fn belly(t:f32)->Vec3 {
    let b=if t>=GULP {((t-GULP)/0.75*std::f32::consts::PI).sin().max(0.0)}else{0.0};
    Vec3::new(1.0+0.22*b,1.0-0.10*b,1.0+0.22*b)
}
pub fn anticipation(t:f32)->f32 {if (0.0..INHALE).contains(&t) {(t/INHALE*std::f32::consts::PI).sin()}else{0.0}}
pub fn root(m:&Mob,t:f32)->Mat4 {
    let inhale=anticipation(t);let size=belly(t)*m.size*Vec3::new(1.0-inhale*0.05,1.0+inhale*0.09,1.0-inhale*0.05);
    let p=m.body.pos-Vec3::Y*if m.sitting {0.15*size.y}else{0.0};
    Mat4::from_translation(p)*Mat4::from_rotation_y(-m.yaw)*Mat4::from_scale(size)
        *Mat4::from_translation(Vec3::Y*0.6)*Mat4::from_rotation_x(inhale*0.23)*Mat4::from_translation(-Vec3::Y*0.6)
}
pub fn mouth_at(m:&Mob,t:f32)->Vec3 {root(m,t).transform_point3(Vec3::new(0.0,0.50,-0.57))}

/// Hold the hungry glance through anticipation even if normal wandering turns it.
pub fn focus(m:&mut Mob,effects:&[Swallow]) {
    if let Some(f)=effects.iter().filter(|f|f.eater==m.id&&f.elapsed<INHALE)
        .min_by(|a,b|a.from.distance_squared(m.body.pos).total_cmp(&b.from.distance_squared(m.body.pos))) {
        let delta=f.from-m.body.pos;if delta.x*delta.x+delta.z*delta.z>0.01 {m.yaw=delta.x.atan2(-delta.z);}
    }
}

impl Game {
    pub(crate) fn swallows_tick(&mut self,dt:f32) {
        let host=!self.is_client();let mut burps=std::collections::BTreeMap::new();
        for f in &mut self.swallows {
            let before=f.elapsed;f.elapsed+=dt.max(0.0);
            if host && before<GULP && f.elapsed>=GULP
                && let Some(m)=self.mobs.iter().find(|m|m.id==f.eater && m.health>0.0) {
                burps.insert(f.eater,mouth_at(m,f.elapsed));
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
            let (pos,scale)=f.pose(mouth_at(m,f.elapsed));
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
    let open=opening(t);
    let face=*root*Mat4::from_rotation_y(std::f32::consts::PI);
    // Hungry pupils draw together while the body leans back for its inhale.
    let focus=(t/INHALE).clamp(0.0,1.0);
    for side in [-1.0,1.0] {
        let eye=Vec3::new(side*0.32-0.065,0.86,0.505);
        g.begin(Pass::Opaque,[1.0,0.96,0.8,1.0],false);
        g.cube(&(face*Mat4::from_translation(eye)*Mat4::from_scale(Vec3::new(0.13,0.13,0.02))),[crate::texture::T_WHITE;6],sky,[0.0,0.0,1.0,1.0]);
        g.begin(Pass::Opaque,[0.12,0.06,0.02,1.0],false);
        g.cube(&(face*Mat4::from_translation(eye+Vec3::new(0.04-side*0.025*focus,0.035,0.025))*Mat4::from_scale(Vec3::new(0.055,0.07,0.02))),[crate::texture::T_WHITE;6],sky,[0.0,0.0,1.0,1.0]);
    }
    if open<=0.0 {return;}
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
