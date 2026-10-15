//! Placed production machines. Contents, energy and mode use the normal saved container record.
use crate::{block::*,containers::Container,game::Game,net::Msg,render::{DynGeo,Pass}};
use macroquad::math::{IVec3,Vec3,Mat4};
use std::collections::{BTreeSet,VecDeque};
pub const STEP:f32=0.25;
pub const CAPACITY:f32=1000.0;
pub const SLOTS:usize=6;
pub const VISUAL_LIMIT:usize=1024;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Kind {Centrifuge,Reactor,Refinery,Oxygen,Solar,Battery,Furnace}
pub fn kind(id:Id)->Option<Kind> {Some(match reg().key_of(id) {
    "half_baked:centrifuge"=>Kind::Centrifuge,"half_baked:reactor"=>Kind::Reactor,
    "ad_absurdum:refinery"=>Kind::Refinery,"ad_absurdum:oxygen_generator"=>Kind::Oxygen,
    "ad_absurdum:solar_panel"=>Kind::Solar,"half_baked:battery"|"ad_absurdum:power_bank"=>Kind::Battery,
    "ad_absurdum:electric_furnace"=>Kind::Furnace,_=>return None,
})}
pub fn cable(id:Id)->bool {matches!(reg().key_of(id),"half_baked:cable"|"ad_absurdum:cable")}
pub fn output(slot:usize)->bool {matches!(slot,2|3|5)}
fn lookup(key:&str)->Option<Id>{reg().lookup(key)}
pub fn hint(id:Id)->&'static str {match kind(id) {
    Some(Kind::Centrifuge)=>"2 dust + coal -> pellet; use coal or cells for power.",
    Some(Kind::Reactor)=>"Pellet + coolant -> 4 cells + waste; 15s; needs tower.",
    Some(Kind::Refinery)=>"2 coal + tank -> 2 fuel; 4s; 8 energy/s.",
    Some(Kind::Oxygen)=>"Battery + tank -> 4 oxygen + empty battery; 8 energy/s.",
    Some(Kind::Solar)=>"Charge empty cells. Clear daylight generates 6 energy/s.",
    Some(Kind::Battery)=>"Store solar power or discharge cells. Charge empty cells.",
    _=>"Smelt one item in 4 powered seconds; 8 energy/s.",
}}
/// Charged portable cells produce stored energy and a reusable empty shell.
pub fn cell(id:Id)->Option<(Id,f32)> {let (empty,energy)=match reg().key_of(id) {
    "half_baked:energy_cell"=>("half_baked:empty_cell",240.0),
    "ad_absurdum:battery"=>("ad_absurdum:empty_battery",120.0),_=>return None,
};Some((lookup(empty)?,energy))}
fn charged(id:Id)->Option<(Id,f32)> {
    for key in ["half_baked:energy_cell","ad_absurdum:battery"] {if let Some(full)=lookup(key)&&let Some((empty,n))=cell(full)&&empty==id{return Some((full,n));}}None
}
pub fn accepts(id:Id,slot:usize,item:Id)->bool {
    let Some(k)=kind(id) else{return false};if output(slot){return false;}
    if slot==4 {if k==Kind::Centrifuge&&item==COAL{return true;}return k!=Kind::Solar&&k!=Kind::Reactor&&cell(item).is_some();}
    let key=reg().key_of(item);
    match (k,slot) {
        (Kind::Centrifuge,0)=>key=="half_baked:glowium_dust",(Kind::Centrifuge,1)=>item==COAL,
        (Kind::Reactor,0)=>key=="half_baked:pellet",(Kind::Reactor,1)=>key=="half_baked:coolant",
        (Kind::Refinery,0)=>item==COAL,(Kind::Refinery,1)=>key=="ad_absurdum:tank",
        (Kind::Oxygen,0)=>key=="ad_absurdum:battery",(Kind::Oxygen,1)=>key=="ad_absurdum:tank",
        (Kind::Solar|Kind::Battery,0)=>charged(item).is_some(),
        (Kind::Furnace,0)=>crate::containers::smelt(item).is_some(),_=>false,
    }
}
#[derive(Clone,Debug)]
struct Recipe {inputs:Vec<(usize,Id,u8)>,outputs:Vec<(usize,Id,u8)>,seconds:f32,energy:f32}
fn recipe(id:Id,c:&Container)->Option<Recipe> {
    let k=kind(id)?;let mut r=Recipe{inputs:Vec::new(),outputs:Vec::new(),seconds:4.0,energy:8.0};
    let (inputs,outputs)=match k {
        Kind::Centrifuge=>(vec![(0,"half_baked:glowium_dust",2),(1,"coal",1)],vec![(2,"half_baked:pellet",1)]),
        Kind::Reactor=>{r.seconds=15.0;r.energy=0.0;(vec![(0,"half_baked:pellet",1),(1,"half_baked:coolant",1)],vec![(2,"half_baked:energy_cell",4),(3,"half_baked:waste",1)])},
        Kind::Refinery=>(vec![(0,"coal",2),(1,"ad_absurdum:tank",1)],vec![(2,"ad_absurdum:fuel",2)]),
        Kind::Oxygen=>(vec![(0,"ad_absurdum:battery",1),(1,"ad_absurdum:tank",1)],vec![(2,"ad_absurdum:oxygen",4),(3,"ad_absurdum:empty_battery",1)]),
        Kind::Solar|Kind::Battery=>{let (empty,_)=c.slots.first().copied().flatten()?;let (full,energy)=charged(empty)?;
            r.inputs=vec![(0,empty,1)];r.outputs=vec![(2,full,1)];r.seconds=4.0;r.energy=energy/4.0;return Some(r);},
        Kind::Furnace=>{let (input,_)=c.slots.first().copied().flatten()?;let out=crate::containers::smelt(input)?;
            r.inputs=vec![(0,input,1)];r.outputs=vec![(2,out,1)];r.seconds=4.0;return Some(r);},
    };
    for (slot,key,n) in inputs {r.inputs.push((slot,lookup(key)?,n));}
    for (slot,key,n) in outputs {r.outputs.push((slot,lookup(key)?,n));}
    Some(r)
}
fn fits(c:&Container,slot:usize,id:Id,n:u8)->bool {
    c.slots.get(slot).is_some_and(|s|match s {None=>true,Some((old,have))=>*old==id&&*have as u16+n as u16<=max_stack(id) as u16&&c.wear[slot]==0})
}
fn put(c:&mut Container,slot:usize,id:Id,n:u8) {c.slots[slot]=Some((id,c.slots[slot].map_or(0,|s|s.1)+n));c.wear[slot]=0;}
fn take(c:&mut Container,slot:usize,n:u8) {if let Some((id,have))=c.slots[slot]{c.slots[slot]=(have>n).then_some((id,have.saturating_sub(n)));if c.slots[slot].is_none(){c.wear[slot]=0;}}}
/// Values are also used on the wire; append new states rather than reordering.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Visual {pub at:IVec3,pub block:Id,pub energy:u16,pub progress:u8,pub mode:u8,pub state:u8}
impl Visual {
    pub fn valid(self)->bool {kind(self.block).is_some()&&self.energy<=1000&&self.mode<=2&&self.state<=9&&(0..crate::world::CH).contains(&self.at.y)}
    pub fn status(self)->&'static str {match self.state {0=>"Stopped",1=>"Ready",2=>"Running",3=>"Missing input",4=>"Missing fuel / coolant",5=>"Output full",6=>"Needs power",7=>"Cooling disconnected - scrammed",8=>"Needs daylight and clear sky",_=>"Storage full"}}
}
fn directions()->[IVec3;6]{[IVec3::X,IVec3::NEG_X,IVec3::Y,IVec3::NEG_Y,IVec3::Z,IVec3::NEG_Z]}
impl Game {
    fn factory_positions(&self)->Vec<IVec3> {
        let mut p:Vec<_>=self.world.containers.keys().copied().filter(|p|self.world.is_loaded(p.x,p.z)&&kind(self.world.get_v(*p)).is_some()).collect();p.sort_by_key(|p|(p.x,p.y,p.z));p
    }
    fn tower(&self,p:IVec3)->bool {(-3..=3).any(|x|(-2..=2).any(|y|(-3..=3).any(|z|reg().key_of(self.world.get_v(p+IVec3::new(x,y,z)))=="half_baked:cooling_tower")))}
    fn sun(&self,p:IVec3)->bool {self.daylight()>0.35&&!matches!(self.realm_dim(),crate::dims::Dim::Scorch|crate::dims::Dim::Hollow)&&((p.y+1)..crate::world::CH).all(|y|!is_solid(self.world.get(p.x,y,p.z)))}
    /// The graph visits at most 256 loaded cells, never loading another chunk or realm.
    fn sources(&self,p:IVec3)->Vec<IVec3> {
        let mut seen=BTreeSet::new();let mut queue=VecDeque::from([p]);let mut sources=Vec::new();
        while let Some(at)=queue.pop_front() {
            if seen.len()>=256{break;}
            if !seen.insert((at.x,at.y,at.z)){continue;}
            if !self.world.is_loaded(at.x,at.z){continue;}
            if at!=p {let id=self.world.get_v(at);if !cable(id)&&!matches!(kind(id),Some(Kind::Solar|Kind::Battery)){continue;}
                if matches!(kind(id),Some(Kind::Solar|Kind::Battery))&&self.world.containers.get(&at).is_some_and(|c|c.burn_total>0.0){sources.push(at);}}
            for d in directions(){let next=at+d;if !seen.contains(&(next.x,next.y,next.z)){queue.push_back(next);}}
        }
        sources
    }
    fn spend_power(&mut self,p:IVec3,n:f32)->bool {
        if n<=0.0{return true;}let sources=self.sources(p);let own=self.world.containers.get(&p).map_or(0.0,|c|c.burn);
        let available=own+sources.iter().filter_map(|p|self.world.containers.get(p)).map(|c|c.burn).sum::<f32>();
        if available+0.0001<n{return false;}
        let mut left=n;for at in std::iter::once(p).chain(sources) {if let Some(c)=self.world.containers.get_mut(&at){let used=left.min(c.burn);c.burn=(c.burn-used).max(0.0);left-=used;if used>0.0 {self.dirty_containers.insert(at);}
            if left<=0.0001{break;}}}true
    }
    pub(crate) fn factory_control(&mut self,from:u32,p:IVec3,action:u8) {
        let valid=if from==self.my_id {self.dead.is_none()&&!self.spectator&&!self.away()&&near(self.player.body.pos,p)}else{self.peers.get(&from).is_some_and(|q|q.alive()&&near(q.target,p))};
        let name=if from==self.my_id {self.player_name.clone()}else{self.peer_name(from)};
        if !valid||kind(self.world.get_v(p)).is_none()||!self.may_build_at(&name,p){return;}
        if from!=self.my_id&&!self.peer_rate_ok(from,"factory_control",0.15){return;}
        self.ensure_container(p);let Some(c)=self.world.containers.get_mut(&p) else{return};
        match action {0=>c.burn_total=1.0,1=>{c.burn_total=0.0;c.cook=0.0;},2=>c.burn_total=if c.burn_total==2.0 {0.0}else{2.0},255=>{},_=>return}
        self.dirty_containers.insert(p);
        if action!=255 {if from==self.my_id{self.advance("factory_floor");}else{self.net_send_to(from,Msg::Advance{key:"factory_floor".into()});}}
        if action==1&&kind(self.world.get_v(p))==Some(Kind::Reactor)&&let Some(mut scripts)=self.scripts.take() {
            // Previously paid script batches retain their original save data until stopped or finished.
            let result=scripts.call(self,"on_chat",vec![name.into(),"/hbm scram".into()]);self.scripts=Some(scripts);
            for error in result.errors{self.msg(format!("Script error {error}"));}self.apply_cmds(result.cmds);
        }
        self.send_container(p,(from!=self.my_id).then_some(from));
        if let Some(v)=self.factory_visual(p){if from==self.my_id{self.factory_states.insert(p,v);}else{self.net_send_to(from,Msg::FactoryStates(vec![v]));}}
    }
    pub fn factory_action(&mut self,action:u8) {let Some(p)=self.open else{return};if self.is_client(){self.net_send_msg(Msg::MachineUse{x:p.x,y:p.y,z:p.z,action});}else{self.factory_control(self.my_id,p,action);}}
    /// Existing production commands now start the nearby placed machine, never pay from a bag.
    pub(crate) fn factory_command(&mut self,args:&[rhai::Dynamic])->bool {
        let [name,text]=args else{return false};let (Some(name),Some(text))=(name.clone().try_cast::<rhai::ImmutableString>(),text.clone().try_cast::<rhai::ImmutableString>())else{return false};
        let (key,action)=match text.trim() {
            "/hbm refine"=>("half_baked:centrifuge",0),"/hbm reactor"=>("half_baked:reactor",0),"/hbm scram"=>("half_baked:reactor",1),"/hbm status"=>("half_baked:reactor",255),
            "/astra refine"=>("ad_absurdum:refinery",0),"/astra refill"=>("ad_absurdum:oxygen_generator",0),"/astra charge"=>("ad_absurdum:solar_panel",0),_=>return false,
        };
        let Some(id)=lookup(key) else{return false};let Some(at)=self.player_position(name.as_str())else{return false};
        let from=if self.is_local_player(name.as_str()){self.my_id}else if let Some(id)=self.peer_by_name(name.as_str()){id}else{return false};
        let p=self.factory_positions().into_iter().filter(|p|self.world.get_v(*p)==id&&near(at,*p)).min_by(|a,b|a.as_vec3().distance_squared(at).total_cmp(&b.as_vec3().distance_squared(at)));
        let message=if let Some(p)=p {self.factory_control(from,p,action);self.factory_visual(p).map_or_else(||"Machine is unavailable.".into(),|v|format!("{}: {}. Ingredients come from its input slots.",item_name(v.block),v.status()))}else{format!("Stand beside a {} and load its input slots.",item_name(id))};
        if from==self.my_id{self.msg(message);}else{self.system_message(Some(from),&message);}true
    }
    pub fn factory_visual(&self,p:IVec3)->Option<Visual> {
        if self.is_client(){return self.factory_states.get(&p).copied().filter(|v|self.world.get_v(p)==v.block);}
        let id=self.world.get_v(p);let k=kind(id)?;let c=self.world.containers.get(&p)?;if c.slots.len()!=SLOTS{return None;}
        let mut state=if c.burn_total<=0.0{if k==Kind::Reactor&&!self.tower(p){7}else{0}}else{1};let mut progress=0;
        if state!=0 {
            if k==Kind::Reactor&&!self.tower(p){state=7;}
            else if let Some(r)=recipe(id,c) {
                progress=(c.cook/r.seconds*255.0).clamp(0.0,255.0) as u8;
                if let Some((slot,_,_))=r.inputs.iter().find(|(slot,id,n)|c.slots[*slot].is_none_or(|s|s.0!=*id||s.1<*n)){state=if *slot==1{4}else{3};}
                else if !r.outputs.iter().all(|(s,id,n)|fits(c,*s,*id,*n)){state=5;}
                else if r.energy>0.0&&c.burn+self.sources(p).iter().filter_map(|p|self.world.containers.get(p)).map(|c|c.burn).sum::<f32>()<r.energy*STEP-0.0001{state=6;}
                else{state=2;}
            }else if matches!(k,Kind::Battery|Kind::Solar) {state=if c.burn>=CAPACITY{9}else if k==Kind::Solar{if self.sun(p){2}else{8}}else{1};}
            else{state=3;}
        }
        Some(Visual{at:p,block:id,energy:c.burn.clamp(0.0,CAPACITY) as u16,progress,mode:c.burn_total.clamp(0.0,2.0) as u8,state})
    }
    pub fn factory_snapshot(&self)->Vec<Visual> {self.factory_positions().into_iter().filter_map(|p|self.factory_visual(p)).take(VISUAL_LIMIT).collect()}
    pub fn factory_tick(&mut self,dt:f32) {
        if self.is_client()||!dt.is_finite()||dt<=0.0{return;}self.factory_timer+=dt;if self.factory_timer<STEP{return;}
        let dt=self.factory_timer.min(1.0);self.factory_timer=0.0;let positions=self.factory_positions();
        for &p in &positions {
            let id=self.world.get_v(p);let k=kind(id).unwrap();let sun=k==Kind::Solar&&self.sun(p);
            let c=self.world.containers.get_mut(&p).unwrap();if c.slots.len()!=SLOTS{continue;}
            if c.burn_total<=0.0{continue;}
            if sun{c.burn=(c.burn+6.0*dt).min(CAPACITY);}
            if k==Kind::Centrifuge&&c.slots[4].is_some_and(|s|s.0==COAL)&&c.burn<=CAPACITY-80.0{take(c,4,1);c.burn+=80.0;}
            if let Some((full,_))=c.slots[4]&&let Some((empty,n))=cell(full)&&c.burn+n<=CAPACITY&&fits(c,5,empty,1){take(c,4,1);put(c,5,empty,1);c.burn+=n;}
        }
        // Banks collect surplus solar energy. Never shuttle energy between banks.
        for &p in &positions {
            if kind(self.world.get_v(p))!=Some(Kind::Battery){continue;}
            let c=&self.world.containers[&p];if c.burn_total<=0.0{continue;}let mut room=(CAPACITY-c.burn).min(20.0*dt);let mut moved=0.0;
            for src in self.sources(p) {if kind(self.world.get_v(src))!=Some(Kind::Solar){continue;}
            if let Some(c)=self.world.containers.get_mut(&src){let n=room.min(c.burn);c.burn-=n;room-=n;moved+=n;}}
            self.world.containers.get_mut(&p).unwrap().burn+=moved;
        }
        for &p in &positions {
            let id=self.world.get_v(p);let k=kind(id).unwrap();
            if k==Kind::Reactor&&!self.tower(p){if let Some(c)=self.world.containers.get_mut(&p)&&c.cook>0.0{c.cook=0.0;c.burn_total=0.0;}continue;}
            let Some(c)=self.world.containers.get(&p) else{continue};if c.slots.len()!=SLOTS||c.burn_total<=0.0{continue;}
            let Some(r)=recipe(id,c) else{continue};
            if !r.inputs.iter().all(|(s,id,n)|c.slots[*s].is_some_and(|v|v.0==*id&&v.1>=*n)){self.world.containers.get_mut(&p).unwrap().cook=0.0;continue;}
            if !r.outputs.iter().all(|(s,id,n)|fits(c,*s,*id,*n)){continue;}
            let work=dt.min((r.seconds-c.cook).max(0.0));if !self.spend_power(p,r.energy*work){continue;}
            let c=self.world.containers.get_mut(&p).unwrap();c.cook+=work;
            if c.cook+0.0001>=r.seconds {for (s,_,n) in r.inputs{take(c,s,n);}for (s,id,n) in r.outputs{put(c,s,id,n);}c.cook=0.0;if c.burn_total==1.0{c.burn_total=0.0;}}
        }
        self.factory_states.clear();for p in positions {self.dirty_containers.insert(p);if let Some(v)=self.factory_visual(p){self.factory_states.insert(p,v);}}
        if self.is_host(){self.net_broadcast(Msg::FactoryStates(self.factory_snapshot()));}
    }
}
fn near(p:Vec3,b:IVec3)->bool {p.is_finite()&&(p+Vec3::Y*1.6).distance(b.as_vec3()+Vec3::splat(0.5))<=10.0}

/// Rotors, status lamps and drifting steam are cosmetic and use host state on clients.
pub fn draw(g:&Game,geo:&mut DynGeo) {
    for (p,v) in &g.factory_states {
        if g.world.get_v(*p)!=v.block||g.player.body.pos.distance(p.as_vec3())>96.0{continue;}
        let at=p.as_vec3();let sky=g.world.sky_shade(p.x,p.y+1,p.z);let active=v.state==2;
        let color=if active {[0.3,1.3,0.5,1.0]}else if v.state==0 {[0.3,0.35,0.4,1.0]}else{[1.3,0.65,0.15,1.0]};
        geo.begin(Pass::Opaque,color,false);let lamp=Mat4::from_translation(at+Vec3::new(0.10,if kind(v.block)==Some(Kind::Solar){0.84}else{0.72},if kind(v.block)==Some(Kind::Reactor){0.878}else if kind(v.block)==Some(Kind::Solar){1.003}else{0.941}))*Mat4::from_scale(Vec3::new(0.12,0.08,0.025));geo.cube(&lamp,[crate::texture::T_WHITE;6],sky,[0.0,0.0,1.0,1.0]);
        if kind(v.block)==Some(Kind::Centrifuge) {
            geo.begin(Pass::Opaque,[1.0;4],false);let rot=Mat4::from_translation(at+Vec3::new(0.5,0.94,0.5))*Mat4::from_rotation_y(if active{g.clock*9.0}else{0.0});
            for lo in [Vec3::new(-0.35,0.0,-0.04),Vec3::new(-0.04,0.0,-0.35)] {let size=if lo.x< -0.1{Vec3::new(0.7,0.06,0.08)}else{Vec3::new(0.08,0.06,0.7)};let m=rot*Mat4::from_translation(lo)*Mat4::from_scale(size);geo.cube(&m,[block(v.block).tex[0];6],sky,[0.0,0.0,1.0,1.0]);}
        }
        if kind(v.block)==Some(Kind::Reactor)&&active
            && let Some(tower)=(-3..=3).flat_map(|x|(-2..=2).flat_map(move |y|(-3..=3).map(move |z|*p+IVec3::new(x,y,z)))).find(|p|reg().key_of(g.world.get_v(*p))=="half_baked:cooling_tower") {
                for i in 0..5 {let t=(g.clock*0.5+i as f32*0.2).fract();geo.begin(Pass::Opaque,[0.75+0.2*t;4],false);let m=Mat4::from_translation(tower.as_vec3()+Vec3::new(0.45+(t*7.0).sin()*0.12,1.1+t*1.5,0.45))*Mat4::from_scale(Vec3::splat(0.10+t*0.15));geo.cube(&m,[crate::texture::T_WHITE;6],sky,[0.0,0.0,1.0,1.0]);}
        }
        if kind(v.block)==Some(Kind::Solar) {geo.begin(Pass::Opaque,[0.25,0.75,1.2,1.0],false);let m=Mat4::from_translation(at+Vec3::new(0.15,1.005,0.15))*Mat4::from_scale(Vec3::new(0.7*(v.energy as f32/CAPACITY),0.015,0.7));geo.cube(&m,[crate::texture::T_WHITE;6],sky,[0.0,0.0,1.0,1.0]);}
    }
}
