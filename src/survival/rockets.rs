//! Script-started autopilot flights, shared by pilot, observers and dedicated hosts.
use crate::{block::*,game::Game,net::Msg,render::{DynGeo,Pass}};
use macroquad::prelude::*;

#[derive(Clone,Debug,PartialEq)]
pub struct RocketFlight {
    pub owner:String,
    pub pilot:u32,
    pub ground:Vec3,
    pub landing:bool,
    pub elapsed:f32,
    pub tier:u8,
}
impl RocketFlight {
    pub fn duration(&self)->f32 {if self.landing {3.0}else{4.0}}
    pub fn pos(&self)->Vec3 {
        let t=(self.elapsed/self.duration()).clamp(0.0,1.0);
        let rise=if self.landing {24.0*(1.0-t)*(1.0-t)}else{48.0*t*t};
        self.ground+Vec3::Y*rise
    }
    pub fn seat(&self)->Vec3 {self.pos()+Vec3::Y*2.0}
    pub fn valid(&self)->bool {self.ground.is_finite() && self.elapsed.is_finite() && (0.0..=4.0).contains(&self.elapsed) && (1..=4).contains(&self.tier) && self.owner.len()<=64}
}
impl Game {
    pub(crate) fn rocket_pilot(&self)->Option<&RocketFlight> {
        if self.away() {return None;}
        self.rockets.iter().find(|f|f.pilot==self.my_id)
    }
    pub(crate) fn rocket_stage(&mut self,owner:&str,landing:bool,ground:Vec3,tier:u8,left:f32) {
        if self.is_client() || !ground.is_finite() {return;}
        let (dim,pilot)=if self.is_local_player(owner) {(self.dim,self.my_id)}else {
            let Some(id)=self.peer_by_name(owner) else{return};
            let Some(dim)=self.peer_dim(id) else{return};(dim,id)
        };
        self.in_realm(dim,|g| {
            if let Some(f)=g.rockets.iter_mut().find(|f|f.owner.eq_ignore_ascii_case(owner) && f.landing==landing) {f.pilot=pilot;return;}
            if g.rockets.len()>=16 {return;}
            let duration=if landing {3.0}else{4.0};
            let f=RocketFlight {owner:owner.into(),pilot,ground,landing,elapsed:(duration-left).clamp(0.0,duration),tier:tier.clamp(1,4)};
            g.rockets.push(f);g.net_broadcast(Msg::RocketFlights(g.rockets.clone()));
        });
    }
    pub(crate) fn rocket_land(&mut self,owner:&str,tier:u8,left:f32) {
        let Some(pos)=self.player_position(owner) else{return};
        let dim=if self.is_local_player(owner) {self.dim}else{let Some(d)=self.peer_by_name(owner).and_then(|id|self.peer_dim(id))else{return};d};
        let ground=self.in_realm(dim,|g| {
            if let Some(f)=g.rockets.iter().find(|f|f.owner.eq_ignore_ascii_case(owner) && f.landing) {return f.ground;}
            let (x,z)=(pos.x.floor() as i32,pos.z.floor() as i32);g.world.load_now(x.div_euclid(16),z.div_euclid(16));
            Vec3::new(pos.x,g.world.surface_y(x,z) as f32+1.1,pos.z)
        });
        // A resumed descent retains its original safe anchor instead of following its airborne pilot.
        self.rocket_stage(owner,true,ground,tier,left);
    }
    pub(crate) fn rocket_clear(&mut self,owner:&str) {
        let dims=self.realm_dims();
        for dim in dims {self.in_realm(dim,|g| {
            let landing=g.rockets.iter().find(|f|f.owner.eq_ignore_ascii_case(owner) && f.landing).map(|f|f.ground);
            g.rockets.retain(|f|!f.owner.eq_ignore_ascii_case(owner));
            if let Some(to)=landing {g.apply_cmds(vec![crate::scripting::Cmd::Teleport(owner.into(),to)]);}
        });}
    }
    pub(crate) fn rockets_tick(&mut self,dt:f32) {
        let host=!self.is_client();let local=if self.away(){None}else{Some(self.my_id)};
        for f in &mut self.rockets {
            if host && local!=Some(f.pilot) && !self.peers.get(&f.pilot).is_some_and(|p|p.alive()) {continue;}
            f.elapsed=(f.elapsed+dt.max(0.0)).min(f.duration());
            let seat=f.seat();
            if local==Some(f.pilot) {self.player.body.pos=seat;self.player.body.vel=Vec3::ZERO;self.player.fall_start=seat.y;}
            if host && let Some(p)=self.peers.get_mut(&f.pilot) {p.pos=seat;p.target=seat;}
        }
    }
    pub(crate) fn draw_rockets(&self,g:&mut DynGeo) {
        for f in &self.rockets {
            if f.pilot==self.my_id && !self.away() && !self.third_person {continue;}
            let at=f.pos()-Vec3::new(0.5,0.0,0.5);
            for (y,key) in [(0,"rocket_engine"),(1,"rocket_body"),(2,"rocket_body"),(3,"rocket_body"),(4,"rocket_nose")] {
                let Some(id)=reg().lookup(&format!("ad_absurdum:{key}"))else{continue};
                let Some(parts)=crate::models::pieces(id)else{continue};g.begin(Pass::Opaque,[1.0;4],false);
                for p in parts {let lo=Vec3::from_array(p.lo);let hi=Vec3::from_array(p.hi);
                    let m=Mat4::from_translation(at+Vec3::Y*y as f32+lo)*Mat4::from_scale(hi-lo);
                    g.cube(&m,p.tiles,1.0,p.uv.unwrap_or([0.0,0.0,1.0,1.0]));
                }
            }
            // Bright nested flame and a moving, fading exhaust column.
            let flame=1.4+0.18*(self.clock*37.0).sin();
            for (width,length,color) in [(0.36,flame,[1.0,0.34,0.04,0.6]),(0.19,flame*0.7,[1.0,0.9,0.35,1.0])] {
                g.begin(if color[3]<1.0 {Pass::Blend}else{Pass::Opaque},color,false);let m=Mat4::from_translation(f.pos()-Vec3::new(width*0.5,length,width*0.5))*Mat4::from_scale(Vec3::new(width,length,width));
                g.cube(&m,[crate::texture::T_WHITE;6],1.0,[0.0,0.0,1.0,1.0]);
            }
            for i in 0..5 {let t=(self.clock*1.8+i as f32*0.2).fract();let width=0.4+t;
                g.begin(Pass::Blend,[0.7,0.73,0.77,(1.0-t)*0.4],false);
                let m=Mat4::from_translation(f.pos()-Vec3::new(width*0.5,flame+t*4.0,width*0.5))*Mat4::from_scale(Vec3::splat(width));
                g.cube(&m,[crate::texture::T_WHITE;6],1.0,[0.0,0.0,1.0,1.0]);
            }
        }
    }
    pub(crate) fn rocket_cockpit(&self) {
        if self.third_person {return;}
        let Some(f)=self.rocket_pilot()else{return};let(w,h)=(screen_width(),screen_height());
        let panel=Color::new(0.07,0.09,0.12,1.0);let trim=Color::new(0.32,0.36,0.4,1.0);
        draw_rectangle(0.0,0.0,w*0.075,h,panel);draw_rectangle(w*0.925,0.0,w*0.075,h,panel);
        draw_rectangle(w*0.075,0.0,w*0.85,h*0.07,trim);draw_rectangle(0.0,h*0.78,w,h*0.22,panel);
        draw_rectangle(w*0.24,h*0.82,w*0.52,h*0.12,Color::new(0.02,0.18,0.14,1.0));
        let phase=if f.landing {"LANDING"}else{"ASCENT"};
        draw_text(format!("AUTOPILOT  |  TIER {}  |  {}",f.tier,phase),w*0.26,h*0.87,22.0,GREEN);
        draw_text(format!("ALT {:.0}   |   {:.1}s   |   F5: exterior view",f.pos().y,(f.duration()-f.elapsed).max(0.0)),w*0.26,h*0.92,19.0,WHITE);
    }
}
