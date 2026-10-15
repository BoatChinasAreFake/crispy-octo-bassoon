//! End-to-end checks of the shipped example packs, read from disk like mods.
use crate::{block::*, dims::Dim, game::Game, mods, scripting::{Cmd,ScriptHost}, world::{GenOptions,Generator}};
use macroquad::math::Vec3;
use rhai::{Dynamic,FLOAT};

pub(crate) fn sources() -> Vec<mods::ModSource> {
    mods::read_disk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example-mods")).into_iter()
        .filter(|s|["half_baked","ad_absurdum","dusk","dubiety"].contains(&s.id.as_str())).collect()
}
fn with_packs(body:impl FnOnce()) {
    let src=sources();assert_eq!(src.len(),4);
    let texts:Vec<_>=src.iter().map(|s|(s.id.as_str(),std::str::from_utf8(&s.files["mod.txt"]).unwrap())).collect();
    mods::with_mods(&texts,|_|body());
}
fn id(key:&str)->Id {reg().lookup(key).unwrap()}
fn dim(key:&str)->Dim {Dim::Mod(reg().dims.iter().position(|d|d.key==key).unwrap() as u8)}
fn game()->Game {
    let mut g=Game::new_with(77,true,false,GenOptions{structures:0,..GenOptions::DEFAULT});g.player_name="Tester".into();
    let (host,errors)=ScriptHost::new(&sources());assert!(errors.is_empty(),"{errors:?}");g.scripts=Some(host);call(&mut g,"on_load",vec![]);
    g.world.load_now(0,0);g.player.body.pos=Vec3::new(4.5,90.0,4.5);g
}
fn call(g:&mut Game,hook:&str,args:Vec<Dynamic>)->Vec<Cmd> {
    let mut host=g.scripts.take().unwrap();let r=host.call(g,hook,args);assert!(r.errors.is_empty(),"{hook}: {:?}",r.errors);g.scripts=Some(host);
    let cmds=r.cmds.clone();g.apply_cmds(r.cmds);cmds
}
fn chat(g:&mut Game,text:&str)->Vec<Cmd>{call(g,"on_chat",vec!["Tester".into(),text.into()])}
fn tick(g:&mut Game,n:usize){for _ in 0..n {call(g,"on_tick",vec![Dynamic::from(1.0 as FLOAT)]);}}
fn place(g:&mut Game,x:i32,y:i32,z:i32,key:&str){g.world.load_now(x.div_euclid(16),z.div_euclid(16));g.world.set_or_record(x,y,z,id(key));}
fn give(g:&mut Game,key:&str,n:u8){g.give(id(key),n);}

#[test]
fn parody_packs_load_together_without_errors_and_fit_the_transfer_budget() {
    let src=sources();let r=mods::build(&src,&[]);
    for m in &r.mods {assert!(m.errors.is_empty(),"{}: {:?}",m.id,m.errors);}
    assert_eq!(r.dims.len(),6);assert_eq!(r.structures.len(),10);assert_eq!(r.mobs.len(),16);
    assert!(src.iter().flat_map(|s|s.files.values()).map(Vec::len).sum::<usize>()<160_000);
    with_packs(||{
        let mut g=game();for help in ["/hbm help","/astra help","/dusk help","/dubiety help"] {assert!(!chat(&mut g,help).is_empty());}
        assert_eq!(reg().dims.iter().find(|d|d.key=="ad_absurdum:moon").unwrap().gravity,0.17);
    });
}

#[test]
fn dusk_terrain_content_never_replaces_the_overworld() {
    let _guard=mods::registry_test_lock();
    let before=Generator::with(77,GenOptions::DEFAULT).generate(0,0);
    drop(_guard);
    let src=sources().into_iter().find(|s|s.id=="dusk").unwrap();
    mods::with_mods(&[("dusk",std::str::from_utf8(&src.files["mod.txt"]).unwrap())],|r|{
        assert!(r.biomes.iter().all(|b|b.dim==Some(0)));assert!(r.structures.iter().all(|s|s.dim==Some(0)));
        assert!(r.ores.iter().all(|o|o.dim==Some(0)));
        assert_eq!(Generator::with(77,GenOptions::DEFAULT).generate(0,0),before);
        let forest=Generator::with_dim(77,GenOptions::DEFAULT,Dim::Mod(0));assert_ne!(forest.generate(0,0),before);
    });
}

#[test]
fn nuclear_processing_requires_machines_and_real_inventory() {
    with_packs(||{
        let mut g=game();chat(&mut g,"/hbm refine");assert_eq!(g.inv.count(id("half_baked:pellet")),0);
        place(&mut g,5,90,4,"half_baked:centrifuge");give(&mut g,"half_baked:glowium_dust",2);give(&mut g,"coal",1);
        chat(&mut g,"/hbm refine");assert_eq!(g.inv.count(id("half_baked:pellet")),1);assert_eq!(g.inv.count(id("half_baked:glowium_dust")),0);
        place(&mut g,3,90,4,"half_baked:reactor");place(&mut g,4,90,5,"half_baked:cooling_tower");give(&mut g,"half_baked:coolant",1);
        chat(&mut g,"/hbm reactor");chat(&mut g,"/hbm reactor");assert_eq!(g.inv.count(id("half_baked:pellet")),0);
        tick(&mut g,16);assert_eq!(g.inv.count(id("half_baked:energy_cell")),4);assert_eq!(g.inv.count(id("half_baked:waste")),1);
        chat(&mut g,"/hbm status");chat(&mut g,"/hbm scram");chat(&mut g,"/hbm geiger");
    });
}

#[test]
fn nuke_countdowns_disarm_and_explode_only_in_the_armed_dimension() {
    with_packs(||{
        let mut g=game();let forest=dim("dusk:twilit_realm");g.move_local_player(forest,Vec3::new(4.5,90.0,4.5));
        place(&mut g,5,90,4,"half_baked:large_nuke");give(&mut g,"half_baked:arming_key",2);
        chat(&mut g,"/hbm arm large");tick(&mut g,4);chat(&mut g,"/hbm disarm");tick(&mut g,12);
        assert_eq!(g.world.get(5,90,4),id("half_baked:large_nuke"));assert_eq!(g.inv.count(id("half_baked:arming_key")),2);
        chat(&mut g,"/hbm arm large");
        g.in_realm(Dim::Over,|g|{g.world.load_now(0,0);g.world.set_or_record(5,90,4,GLASS);});
        g.move_local_player(Dim::Over,Vec3::new(100.0,90.0,100.0));let cmds=call(&mut g,"on_tick",vec![Dynamic::from(10.0 as FLOAT)]);
        // Each tick is bounded to one second; a delayed event never detonates ten times.
        assert!(!cmds.iter().any(|c|matches!(c,Cmd::InRealm(_,c) if matches!(**c,Cmd::Explode(..)))));
        let mut blasts=0;for _ in 0..10 {blasts+=call(&mut g,"on_tick",vec![Dynamic::from(1.0 as FLOAT)]).iter().filter(|c|matches!(c,Cmd::InRealm(_,c) if matches!(**c,Cmd::Explode(..)))).count();}
        assert_eq!(blasts,9);assert_eq!(g.world.get(5,90,4),GLASS);
        g.in_realm(forest,|g|assert_eq!(g.world.get(5,90,4),AIR));assert_eq!(g.realm_dim(),Dim::Over);
    });
}

#[test]
fn rockets_charge_fuel_recover_gear_and_land_in_a_separate_planet() {
    with_packs(||{
        let mut g=game();give(&mut g,"ad_absurdum:rocket_1",1);give(&mut g,"ad_absurdum:fuel",2);give(&mut g,"ad_absurdum:life_support",1);give(&mut g,"ad_absurdum:oxygen",2);
        g.inv.armor[0]=Some((id("ad_absurdum:space_helmet"),1));g.inv.armor[1]=Some((id("ad_absurdum:space_chestplate"),1));
        place(&mut g,5,89,4,"ad_absurdum:launch_pad");place(&mut g,3,90,4,"ad_absurdum:rocket_workbench");
        for y in 90..95 {g.world.set_or_record(5,y,4,AIR);}
        chat(&mut g,"/astra assemble 1");assert_eq!(g.inv.count(id("ad_absurdum:rocket_1")),0);
        chat(&mut g,"/astra launch mars");assert_eq!(g.inv.count(id("ad_absurdum:fuel")),2,"higher-tier destination rejected");
        chat(&mut g,"/astra launch moon");assert_eq!(g.inv.count(id("ad_absurdum:fuel")),1);tick(&mut g,2);
        let mut back=Game::from_save(g.to_save());back.player_name="Tester".into();back.start_scripts_with(&sources());
        assert!(back.rockets.is_empty());tick(&mut back,1);assert_eq!(back.rocket_pilot().unwrap().elapsed,2.0);
        g=back;tick(&mut g,1);
        assert_eq!(g.dim,dim("ad_absurdum:moon"));assert!(g.rockets.iter().any(|f|f.landing));assert_eq!(g.inv.count(id("ad_absurdum:rocket_1")),0);
        tick(&mut g,3);assert!(g.rockets.is_empty());assert_eq!(g.inv.count(id("ad_absurdum:rocket_1")),1);
        assert_eq!(g.inv.count(id("ad_absurdum:launch_pad")),1);assert_eq!(g.inv.count(id("ad_absurdum:rocket_workbench")),1);
        g.in_realm(Dim::Over,|g|{assert_eq!(g.world.get(5,89,4),AIR);assert_eq!(g.world.get(3,90,4),AIR);});
        assert!(g.world.get(g.player.body.pos.x.floor() as i32,g.player.body.pos.y.floor() as i32-1,g.player.body.pos.z.floor() as i32)!=AIR);
        tick(&mut g,1);assert_eq!(g.inv.count(id("ad_absurdum:oxygen")),1);assert_eq!(g.inv.count(id("ad_absurdum:tank")),1);
        chat(&mut g,"/astra status");
    });
}

#[test]
fn dusk_guardians_gate_progress_and_reward_cooperation() {
    with_packs(||{
        let mut g=game();let forest=dim("dusk:twilit_realm");g.move_local_player(forest,Vec3::new(4.5,90.0,4.5));
        place(&mut g,5,90,4,"dusk:lich_altar");chat(&mut g,"/dusk challenge lich");assert!(g.mobs.iter().all(|m|m.kind!=crate::entity::MobKind::from_name("dusk:lich").unwrap()));
        place(&mut g,5,90,4,"dusk:serpent_altar");chat(&mut g,"/dusk challenge serpent");let n=g.mobs.len();assert!(n>0);chat(&mut g,"/dusk challenge serpent");assert_eq!(g.mobs.len(),n);
        call(&mut g,"on_mob_death",vec!["dusk:serpent".into(),5.into(),91.into(),4.into(),"Friend".into()]);
        assert_eq!(g.board.score("dusk_progress","Tester"),1);assert_eq!(g.board.score("dusk_progress","Friend"),1);
        place(&mut g,5,90,4,"dusk:lich_altar");chat(&mut g,"/dusk challenge lich");assert!(g.mobs.iter().any(|m|m.kind==crate::entity::MobKind::from_name("dusk:lich").unwrap()));
        g.move_local_player(Dim::Over,Vec3::new(4.5,90.0,4.5));tick(&mut g,10);assert!(g.mobs.iter().all(|m|!m.kind.mod_def().is_some_and(|d|d.key.starts_with("dusk:"))));
    });
}

#[test]
fn dubiety_generates_bounded_replies_and_saves_private_notes() {
    with_packs(||{
        let mut g=game();give(&mut g,"dubiety:beacon",1);give(&mut g,"dubiety:chip",2);
        chat(&mut g,"/dubiety remember Build beside the little beach");let mut outputs=std::collections::HashSet::new();
        for prompt in ["mine iron","cook food","space oxygen","dusk guardian","build house","fight boss","travel home","reactor coolant","hello"] {
            for _ in 0..6 {for c in chat(&mut g,&format!("/dubiety ask {prompt}")) {if let Cmd::Message(_,text)=c && text.starts_with("Dubiety:") {assert!(text.split_whitespace().count()<=29);outputs.insert(text);}}}
        }
        assert!(outputs.len()>12,"model never varies its word sequences");
        let (saved,errors)=g.scripts.as_ref().unwrap().export_vars();assert!(errors.is_empty(),"{errors:?}");assert!(saved.iter().map(|(_,b)|b.len()).sum::<usize>()<100_000);
        let (mut fresh,errors)=ScriptHost::new(&sources());assert!(errors.is_empty());assert!(fresh.import_vars(&saved).is_empty());g.scripts=Some(fresh);call(&mut g,"on_load",vec![]);
        assert!(chat(&mut g,"/dubiety recall").iter().any(|c|matches!(c,Cmd::Message(_,text) if text.contains("little beach"))));
        let mut peer=crate::multiplayer::Peer::new("Another".into(),g.player.body.pos);peer.ledger.bag.add(id("dubiety:beacon"),1);g.peers.insert(201,peer);
        let notes=call(&mut g,"on_chat",vec!["Another".into(),"/dubiety recall".into()]);assert!(!notes.iter().any(|c|matches!(c,Cmd::Message(_,text) if text.contains("little beach"))));
        chat(&mut g,"/dubiety summon");assert!(g.mobs.iter().any(|m|m.kind==crate::entity::MobKind::from_name("dubiety:companion").unwrap()));
        chat(&mut g,"/dubiety forget");assert!(!chat(&mut g,"/dubiety recall").iter().any(|c|matches!(c,Cmd::Message(_,text) if text.contains("little beach"))));
    });
}

#[test]
fn planetary_player_gravity_and_example_advancements_work() {
    with_packs(||{
        let mut g=game();g.player.body.pos=Vec3::new(4.5,200.0,4.5);g.player.body.vel=Vec3::ZERO;
        g.player.update(0.1,&crate::player::Input::default(),&g.world,false);let earth=g.player.body.vel.y;
        g.move_local_player(dim("ad_absurdum:moon"),Vec3::new(4.5,200.0,4.5));g.world.load_now(0,0);
        g.player.update(0.1,&crate::player::Input::default(),&g.world,false);
        assert!((g.player.body.vel.y-earth*0.17).abs()<0.001);
        for (item,key) in [("half_baked:energy_cell","half_baked_power"),("ad_absurdum:rocket_1","rocket_paperwork"),("dubiety:beacon","pocket_philosopher"),("dusk:hydra_heart","complaint_resolved")] {
            g.item_advancements(id(item));assert!(g.advancements.earned.iter().any(|a|a==key));
        }
    });
}

#[test]
fn mod_gravity_defaults_and_non_finite_values_are_safe() {
    let text="[block frame]\ntexture = stone\n[dimension old]\nframe = frame\n[dimension tiny]\nframe = frame\ngravity = 0\n[dimension bad]\nframe = frame\ngravity = NaN\n";
    let mut files=std::collections::BTreeMap::new();files.insert("mod.txt".into(),text.as_bytes().to_vec());
    let r=mods::build(&[mods::ModSource{id:"gravity".into(),files}],&[]);
    assert_eq!(r.dims.iter().find(|d|d.key=="gravity:old").unwrap().gravity,1.0);
    assert_eq!(r.dims.iter().find(|d|d.key=="gravity:tiny").unwrap().gravity,0.15);
    assert_eq!(r.dims.iter().find(|d|d.key=="gravity:bad").unwrap().gravity,1.0);
    assert!(r.mods[0].errors.iter().any(|e|e.contains("gravity must be finite")));
}

#[test]
fn cooling_loss_melts_a_reactor_and_unloaded_countdowns_pause() {
    with_packs(||{
        let mut g=game();place(&mut g,3,90,4,"half_baked:reactor");place(&mut g,4,90,5,"half_baked:cooling_tower");
        give(&mut g,"half_baked:pellet",1);give(&mut g,"half_baked:coolant",1);chat(&mut g,"/hbm reactor");
        g.world.set_or_record(4,90,5,AIR);let cmds=call(&mut g,"on_tick",vec![Dynamic::from(1.0 as FLOAT)]);
        assert!(cmds.iter().any(|c|matches!(c,Cmd::InRealm(_,c) if matches!(**c,Cmd::Explode(..)))));
        assert_eq!(g.inv.count(id("half_baked:energy_cell")),0);
        g.player.body.pos=Vec3::new(4.5,90.0,4.5);place(&mut g,5,90,4,"half_baked:small_nuke");give(&mut g,"half_baked:arming_key",1);chat(&mut g,"/hbm arm small");
        let chunk=g.world.chunks.remove(&(0,0)).unwrap();tick(&mut g,12);g.world.chunks.insert((0,0),chunk);
        let cmds=call(&mut g,"on_tick",vec![Dynamic::from(1.0 as FLOAT)]);
        assert!(!cmds.iter().any(|c|matches!(c,Cmd::InRealm(_,c) if matches!(**c,Cmd::Explode(..)))));
        assert_eq!(g.world.get(5,90,4),id("half_baked:small_nuke"));
    });
}

#[test]
fn pack_notes_reactor_progress_and_realms_survive_a_game_save() {
    with_packs(||{
        let mut g=game();give(&mut g,"dubiety:beacon",1);chat(&mut g,"/dubiety remember Bring coolant home");
        place(&mut g,3,90,4,"half_baked:reactor");place(&mut g,4,90,5,"half_baked:cooling_tower");
        give(&mut g,"half_baked:pellet",1);give(&mut g,"half_baked:coolant",1);chat(&mut g,"/hbm reactor");tick(&mut g,5);
        g.move_local_player(dim("ad_absurdum:moon"),Vec3::new(4.5,90.0,4.5));
        let dir=std::env::temp_dir().join(format!("minceraft-parody-save-{}",std::process::id()));std::fs::create_dir_all(&dir).unwrap();let path=dir.join("world.mncr");
        crate::save::write_to(&path,&g.to_save()).unwrap();let mut back=Game::from_save(crate::save::read_from(&path).unwrap());back.player_name="Tester".into();back.start_scripts_with(&sources());
        assert_eq!(back.dim,dim("ad_absurdum:moon"));assert_eq!(back.player_name,"Tester");
        assert!(chat(&mut back,"/dubiety recall").iter().any(|c|matches!(c,Cmd::Message(_,text) if text.contains("coolant home"))));
        back.in_realm(Dim::Over,|g|g.world.load_now(0,0));tick(&mut back,10);
        assert_eq!(back.inv.count(id("half_baked:energy_cell")),4);assert_eq!(back.realm_dim(),dim("ad_absurdum:moon"));
        std::fs::remove_dir_all(dir).unwrap();
    });
}

#[test]
fn rocket_shapes_have_taper_fins_bounded_geometry_and_inventory_models() {
    with_packs(||{
        for (key,shape) in [("rocket_engine",Shape::RocketEngine),("rocket_body",Shape::RocketBody),("rocket_nose",Shape::RocketNose)] {
            let id=id(&format!("ad_absurdum:{key}"));assert_eq!(block(id).shape,shape);assert_eq!(block(id).model,Model::Shaped);
            let parts=crate::models::pieces(id).unwrap();assert!(!parts.is_empty());assert_eq!(crate::models::item_parts(id).len(),parts.len());
            for p in &parts {for axis in 0..3 {assert!(p.lo[axis]>=0.0 && p.hi[axis]<=1.0 && p.lo[axis]<p.hi[axis]);}}
            let (boxes,n)=shape.boxes();assert_eq!(n,3);
            for p in &parts {
                // Every rendered point is inside at least one collision box.
                for corner in 0..8 {let point=std::array::from_fn::<_,3,_>(|axis|if corner & (1<<axis)==0 {p.lo[axis]}else{p.hi[axis]});
                    assert!(boxes[..n].iter().any(|(a,b)|(0..3).all(|axis|point[axis]>=a[axis]-0.00001 && point[axis]<=b[axis]+0.00001)),"unbounded {key} geometry {point:?}");}
            }
        }
        let parts=crate::models::pieces(id("ad_absurdum:rocket_nose")).unwrap();
        assert!(parts.windows(2).all(|p|p[1].hi[0]-p[1].lo[0]<p[0].hi[0]-p[0].lo[0]));
        let fins=crate::models::pieces(id("ad_absurdum:rocket_engine")).unwrap();assert_eq!(fins.len(),16);
        for axis in [0,2] {assert!(fins.iter().any(|p|p.lo[axis]==0.0));assert!(fins.iter().any(|p|p.hi[axis]==1.0));}
    });
}

#[test]
fn dusk_custom_trees_have_three_distinct_shapes_and_stable_chunk_edges() {
    with_packs(||{
        let g=Generator::with_dim(77,GenOptions::DEFAULT,dim("dusk:twilit_realm"));
        let trees:Vec<_>=g.biomes.iter().filter(|b|b.key.starts_with("dusk:")).filter_map(|b|b.tree.as_ref()).collect();
        assert_eq!(trees.len(),3);
        assert_eq!(trees.iter().map(|t|t.style).collect::<Vec<_>>(),vec![crate::modgen::ModTreeStyle::Twisted,crate::modgen::ModTreeStyle::Spire,crate::modgen::ModTreeStyle::Umbrella]);
        for tree in trees {
            let blocks=g.custom_tree_blocks(15,15,70,tree);assert_eq!(blocks,g.custom_tree_blocks(15,15,70,tree));
            assert!(blocks.iter().any(|(p,id,_)|*id==tree.log && p.y>70));
            assert!(blocks.iter().any(|(p,id,_)|*id==tree.leaves && p.x>=16));
            assert!(blocks.iter().any(|(p,id,_)|*id==tree.leaves && p.x<16));
            assert!(blocks.iter().all(|(p,_,_)|(p.x-15).abs()<=6 && (p.z-15).abs()<=6));
            assert!(block(tree.leaves).solid && !block(tree.leaves).opaque);
        }
        let chunks=[(-1,-1),(0,0),(1,0),(0,1)];let first:Vec<_>=chunks.iter().map(|&(x,z)|g.generate(x,z)).collect();
        for (i,&(x,z)) in chunks.iter().enumerate().rev() {assert_eq!(first[i],g.generate(x,z));}
        for key in ["dusk:duskwood_planks","dusk:gloamwood_planks","dusk:moonwillow_planks"] {assert!(reg().lookup(key).is_some());}
    });
}


#[test]
fn obesity_summons_eats_every_offering_and_saves_as_a_companion() {
    with_packs(||{
        use crate::{animals::Interaction,entity::MobKind};
        let mut g=game();g.creative=false;g.mobs.clear();
        give(&mut g,"dubiety:beacon",1);chat(&mut g,"/dubiety obesity");
        assert!(g.mobs.is_empty());
        give(&mut g,"dubiety:obesity_beacon",1);chat(&mut g,"/dubiety obesity");assert!(g.mobs.is_empty());
        give(&mut g,"dubiety:chip",2);chat(&mut g,"/dubiety obesity");
        let kind=MobKind::from_name("dubiety:obesity").unwrap();
        assert_eq!(g.mobs.len(),1);assert_eq!(g.mobs[0].kind,kind);assert_eq!(g.inv.count(id("dubiety:chip")),1);
        for item in [id("dubiety:chip"),STONE,NAME_TAG,LEAD,SHEARS,PICK_DIAMOND,TNT,id("half_baked:waste"),WHEAT] {
            if g.inv.count(item)==0 {g.give(item,1);}
            let before=g.inv.count(item);g.inv.selected=g.inv.slots.iter().position(|s|s.is_some_and(|s|s.0==item)).unwrap();
            assert!(g.use_on_mob(0));assert_eq!(g.inv.count(item),before-1,"offering {}",block(item).key);
            assert!(g.naming.is_none());assert!(g.mobs[0].leash.is_none());
        }
        assert_eq!(g.mobs[0].owner.as_deref(),Some("tester"));
        let mob=g.mobs[0].id;let at=g.player.body.pos;
        assert_eq!(g.interact_mob("tester",at,mob,AIR),Interaction::Toggled);
        assert_eq!(g.interact_mob("tester",at+Vec3::X*30.0,mob,STONE),Interaction::Nothing);
        let back=Game::from_save(g.to_save());
        assert!(back.mobs.iter().any(|m|m.kind==kind && m.owner.as_deref()==Some("tester") && m.persistent));
    });
}

#[test]
fn obesity_swallows_only_livestock_in_its_five_block_sphere_and_keeps_demanding() {
    with_packs(||{
        use crate::entity::MobKind;
        let mut g=game();g.mobs.clear();let at=Vec3::new(8.0,90.0,8.0);g.player.body.pos=at;
        let kind=MobKind::from_name("dubiety:obesity").unwrap();let glutton=g.alloc_mob(kind,at);
        for (k,delta) in [(MobKind::Mooer,Vec3::X*2.0),(MobKind::Oinker,-Vec3::X*5.0),(MobKind::Fluffer,Vec3::new(0.0,3.0,4.0)),(MobKind::Cluckster,-Vec3::Y*5.0)] {
            let id=g.alloc_mob(k,at+delta);let m=g.mobs.iter_mut().find(|m|m.id==id).unwrap();m.persistent=true;m.baby=30.0;
        }
        let outside=g.alloc_mob(MobKind::Mooer,at+Vec3::X*5.01);
        let above=g.alloc_mob(MobKind::Cluckster,at+Vec3::Y*5.01);
        let wolf=g.alloc_mob(MobKind::Woofer,at);let assistant=g.alloc_mob(MobKind::from_name("dubiety:companion").unwrap(),at);
        let dead=g.alloc_mob(MobKind::Mooer,at);g.mobs.iter_mut().find(|m|m.id==dead).unwrap().health=0.0;
        let forest=dim("dusk:twilit_realm");let other=g.in_realm(forest,|g|g.alloc_mob(MobKind::Oinker,at));
        g.messages.clear();let drops=g.drops.len();let kills=g.stats.kills;
        g.gluttons_tick(0.1);
        assert_eq!(g.mobs.iter().map(|m|m.id).collect::<Vec<_>>(),vec![glutton,outside,above,wolf,assistant,dead]);
        assert_eq!(g.drops.len(),drops);assert_eq!(g.stats.kills,kills);
        assert!(g.messages.iter().any(|m|m.0.contains("FEED ME!")));
        let n=g.messages.len();g.gluttons_tick(7.0);assert_eq!(g.messages.len(),n);g.gluttons_tick(1.0);assert_eq!(g.messages.len(),n+1);
        g.in_realm(forest,|g|assert!(g.mobs.iter().any(|m|m.id==other)));
        let bytes=crate::animals::encode_mobs(&g.mobs,&std::collections::HashMap::new());let back=crate::animals::decode_mobs(&bytes,&mut g.rng);
        assert!(!back.iter().any(|m|matches!(m.kind,MobKind::Oinker|MobKind::Fluffer)));
        g.mobs.iter_mut().find(|m|m.id==glutton).unwrap().health=0.0;
        g.mobs.iter_mut().find(|m|m.id==outside).unwrap().body.pos=at;g.gluttons_tick(8.0);
        assert!(g.mobs.iter().any(|m|m.id==outside));
    });
}


#[test]
fn rocket_snapshots_attach_pilots_and_descent_reuses_its_safe_anchor() {
    with_packs(||{
        let mut g=game();g.mobs.clear();let ground=Vec3::new(4.5,90.0,4.5);
        g.rocket_stage("Tester",false,ground,2,4.0);
        g.rockets_tick(1.0);let first=g.player.body.pos;
        g.rockets_tick(1.0);assert!(g.player.body.pos.y>first.y);
        assert_eq!(g.player.body.pos,g.rocket_pilot().unwrap().seat());assert_eq!(g.player.body.vel,Vec3::ZERO);
        let msg=crate::net::Msg::RocketFlights(g.rockets.clone());
        assert_eq!(crate::net::Msg::decode(&msg.encode()).unwrap(),msg);
        let mut bad=g.rockets.clone();bad[0].ground.y=f32::NAN;assert!(crate::net::Msg::decode(&crate::net::Msg::RocketFlights(bad).encode()).is_err());
        // A script-started phase reconstructed after load begins at its saved remaining time.
        g.rocket_clear("Tester");g.rocket_stage("Tester",false,ground,2,2.0);
        assert_eq!(g.rocket_pilot().unwrap().elapsed,2.0);assert_eq!(g.rocket_pilot().unwrap().pos().y,ground.y+12.0);
        g.rocket_clear("Tester");g.rocket_land("Tester",2,3.0);
        let high=g.rocket_pilot().unwrap().pos().y;g.rockets_tick(1.5);
        assert!(g.rocket_pilot().unwrap().pos().y<high);let anchor=g.rocket_pilot().unwrap().ground;
        g.rocket_land("Tester",2,1.5);assert_eq!(g.rocket_pilot().unwrap().ground,anchor);
        g.rockets_tick(1.5);assert_eq!(g.rocket_pilot().unwrap().pos(),anchor);
        g.rocket_clear("Tester");assert!(g.rocket_pilot().is_none());assert_eq!(g.player.body.pos,anchor);
    });
}

#[test]
fn swallowing_is_visual_only_bounded_and_realm_local() {
    with_packs(||{
        use crate::{entity::MobKind,swallow::{self,Swallow}};
        let mut g=game();g.mobs.clear();let at=g.player.body.pos;
        let eater=g.alloc_mob(MobKind::from_name("dubiety:obesity").unwrap(),at);
        let prey=g.alloc_mob(MobKind::Fluffer,at+Vec3::Z*3.0);
        let sheep=g.mobs.iter_mut().find(|m|m.id==prey).unwrap();sheep.baby=30.0;sheep.sheared=true;
        let expected=Swallow::meal(eater,sheep);let drops=g.drops.len();let kills=g.stats.kills;
        g.gluttons_tick(0.016);
        assert_eq!(g.swallows,vec![expected]);assert!(!g.mobs.iter().any(|m|m.id==prey));
        assert_eq!(g.drops.len(),drops);assert_eq!(g.stats.kills,kills);
        let mouth=swallow::mouth_at(&g.mobs[0],0.0);let (start,scale)=g.swallows[0].pose(mouth);
        let mut waiting=g.swallows[0].clone();waiting.elapsed=swallow::INHALE*0.5;
        let (held,held_scale)=waiting.pose(swallow::mouth_at(&g.mobs[0],waiting.elapsed));
        assert!(held.distance(start)<0.0001);assert_eq!(held_scale,scale);
        assert!(swallow::anticipation(waiting.elapsed)>0.99);assert_eq!(swallow::anticipation(swallow::INHALE),0.0);
        assert!(swallow::root(&g.mobs[0],waiting.elapsed).transform_point3(Vec3::Y).distance(swallow::root(&g.mobs[0],0.0).transform_point3(Vec3::Y))>0.05);

        assert!(start.is_finite());assert_eq!(scale,Vec3::splat(0.55));
        let msg=crate::net::Msg::Swallows(g.swallows.clone());assert_eq!(crate::net::Msg::decode(&msg.encode()).unwrap(),msg);
        assert!(g.world_msgs().contains(&msg));
        let mut bad=g.swallows.clone();bad[0].elapsed=f32::NAN;
        assert!(crate::net::Msg::decode(&crate::net::Msg::Swallows(bad).encode()).is_err());
        let mut bad=g.swallows.clone();bad[0].kind=MobKind::Woofer.index();
        assert!(crate::net::Msg::decode(&crate::net::Msg::Swallows(bad).encode()).is_err());
        let forest=dim("dusk:twilit_realm");
        g.in_realm(forest,|g|{assert!(g.swallows.is_empty());g.swallows_tick(5.0);});assert_eq!(g.swallows.len(),1);
        g.sounds.clear();g.swallows_tick(swallow::GULP);
        let (end,scale)=g.swallows[0].pose(mouth);assert!(end.distance(mouth)<0.0001);assert_eq!(scale,Vec3::ZERO);
        assert_eq!(g.sounds.iter().filter(|(s,_)|*s==crate::sound::Sfx::Burp).count(),1);
        g.swallows_tick(0.1);assert_eq!(g.sounds.iter().filter(|(s,_)|*s==crate::sound::Sfx::Burp).count(),1);
        // The cosmetic meal is intentionally absent from saves; swallowed livestock stays gone.
        let back=Game::from_save(g.to_save());assert!(back.swallows.is_empty());assert!(!back.mobs.iter().any(|m|m.id==prey));
        g.swallows_tick(2.0);assert!(g.swallows.is_empty());
        for _ in 0..swallow::CAP+4 {g.alloc_mob(MobKind::Mooer,at);}
        g.gluttons_tick(0.016);assert_eq!(g.swallows.len(),swallow::CAP);assert_eq!(g.mobs.len(),1);
        assert_eq!(swallow::opening(0.0),0.0);assert!(swallow::opening(0.6)>0.99);
        assert_eq!(swallow::opening(swallow::DURATION),0.0);
        assert!(swallow::belly(swallow::GULP+0.35).x>1.1);assert!((swallow::belly(swallow::DURATION)-Vec3::ONE).length()<0.001);
    });
}

fn factory_steps(g:&mut Game,n:usize){for _ in 0..n {g.factory_tick(0.25);}}
fn stock(g:&mut Game,p:macroquad::math::IVec3,slot:usize,key:&str,n:u8){g.world.containers.get_mut(&p).unwrap().slots[slot]=Some((id(key),n));}
#[test]
fn native_machine_controls_use_block_stock_and_reject_remote_or_invalid_actions() {with_packs(||{
    let mut g=game();let p=macroquad::math::IVec3::new(4,90,5);place(&mut g,p.x,p.y,p.z,"half_baked:centrifuge");
    stock(&mut g,p,0,"half_baked:glowium_dust",4);stock(&mut g,p,1,"coal",2);stock(&mut g,p,4,"half_baked:energy_cell",1);
    give(&mut g,"half_baked:glowium_dust",1);assert!(g.open_machine(p));assert!(!g.fire("on_chat",vec!["Tester".into(),"/hbm refine".into()]));factory_steps(&mut g,16);assert_eq!(g.inv.count(id("half_baked:glowium_dust")),1);
    let c=&g.world.containers[&p];assert_eq!(c.slots[2],Some((id("half_baked:pellet"),1)));assert_eq!(c.slots[0].unwrap().1,2);
    assert_eq!(c.slots[5],Some((id("half_baked:empty_cell"),1)));assert!((c.burn-208.0).abs()<0.01);assert_eq!(g.inv.count(id("half_baked:pellet")),0);
    g.factory_action(200);g.player.body.pos.x+=40.0;g.factory_action(0);factory_steps(&mut g,20);assert_eq!(g.world.containers[&p].slots[2].unwrap().1,1);
    g.player.body.pos=Vec3::new(4.5,90.0,4.5);g.dead=Some("test".into());g.factory_action(0);assert_eq!(g.world.containers[&p].burn_total,0.0);
});}
#[test]
fn factory_reactor_blocks_overflow_and_scrams_without_losing_unprocessed_inputs() {with_packs(||{
    let mut g=game();let p=macroquad::math::IVec3::new(4,90,5);place(&mut g,4,90,5,"half_baked:reactor");place(&mut g,5,90,5,"half_baked:cooling_tower");
    stock(&mut g,p,0,"half_baked:pellet",2);stock(&mut g,p,1,"half_baked:coolant",2);stock(&mut g,p,2,"half_baked:energy_cell",64);
    g.open_machine(p);g.factory_action(2);factory_steps(&mut g,16);assert_eq!(g.factory_visual(p).unwrap().state,5);assert_eq!(g.world.containers[&p].cook,0.0);
    g.world.containers.get_mut(&p).unwrap().slots[2]=None;factory_steps(&mut g,12);assert!((g.world.containers[&p].cook-3.0).abs()<0.01);
    g.world.set(5,90,5,AIR);factory_steps(&mut g,1);assert_eq!(g.world.containers[&p].burn_total,0.0);assert_eq!(g.factory_visual(p).unwrap().state,7);assert_eq!(g.world.containers[&p].slots[0].unwrap().1,2);
    place(&mut g,5,90,5,"half_baked:cooling_tower");g.factory_action(0);factory_steps(&mut g,60);
    assert_eq!(g.world.containers[&p].slots[2],Some((id("half_baked:energy_cell"),4)));assert_eq!(g.world.containers[&p].slots[3],Some((id("half_baked:waste"),1)));assert_eq!(g.world.containers[&p].slots[1].unwrap().1,1);
});}
#[test]
fn factory_hoppers_feed_automatic_batches_and_extract_only_finished_outputs() {with_packs(||{
    use macroquad::math::IVec3;
    let mut g=game();let p=IVec3::new(4,90,4);place(&mut g,4,90,4,"half_baked:centrifuge");
    for (pos,block) in [(p+IVec3::Y,HOPPER_FIRST),(p+IVec3::Y*2,CHEST),(p-IVec3::Y,HOPPER_FIRST),(p-IVec3::Y*2,CHEST)]{g.world.set_v(pos,block);}
    stock(&mut g,p+IVec3::Y*2,0,"half_baked:glowium_dust",4);stock(&mut g,p+IVec3::Y*2,1,"coal",2);stock(&mut g,p,4,"half_baked:energy_cell",1);
    g.open_machine(p);g.factory_action(2);for _ in 0..160{g.hoppers_tick(0.25);g.factory_tick(0.25);}
    let chest=&g.world.containers[&(p-IVec3::Y*2)];assert!(chest.slots.contains(&Some((id("half_baked:pellet"),2))));
    assert!(chest.slots.iter().flatten().all(|s|matches!(reg().key_of(s.0),"half_baked:pellet"|"half_baked:empty_cell")));assert_eq!(g.world.containers[&p].burn_total,2.0);
});}
#[test]
fn factory_cables_conserve_power_and_disconnects_pause_processing() {with_packs(||{
    use macroquad::math::IVec3;
    let mut g=game();let bank=IVec3::new(4,90,4);let wire=bank+IVec3::X;let oven=wire+IVec3::X;
    place(&mut g,4,90,4,"ad_absurdum:power_bank");place(&mut g,5,90,4,"ad_absurdum:cable");place(&mut g,6,90,4,"ad_absurdum:electric_furnace");
    g.world.containers.get_mut(&bank).unwrap().burn=240.0;stock(&mut g,oven,0,"sand",2);g.world.containers.get_mut(&oven).unwrap().burn_total=2.0;
    factory_steps(&mut g,16);assert_eq!(g.world.containers[&oven].slots[2],Some((GLASS,1)));assert!((g.world.containers[&bank].burn-208.0).abs()<0.001);
    g.world.set_v(wire,AIR);factory_steps(&mut g,16);assert_eq!(g.factory_visual(oven).unwrap().state,6);assert_eq!(g.world.containers[&oven].slots[2].unwrap().1,1);
    place(&mut g,5,90,4,"ad_absurdum:cable");factory_steps(&mut g,16);assert_eq!(g.world.containers[&oven].slots[2],Some((GLASS,2)));assert!((g.world.containers[&bank].burn-176.0).abs()<0.001);
    let before=g.world.containers[&bank].clone();let state=crate::containers::decode(&crate::containers::encode(&g.world.containers),4);assert_eq!(state[&bank],before);
    let mut back=Game::from_save(g.to_save());back.world.load_now(0,0);assert_eq!(back.world.containers[&bank],before);assert_eq!(back.world.containers[&oven].slots[2],Some((GLASS,2)));
    let dim=dim("ad_absurdum:moon");g.in_realm(dim,|g|{g.world.load_now(0,0);g.world.set_v(oven,id("ad_absurdum:electric_furnace"));stock(g,oven,0,"sand",1);g.world.containers.get_mut(&oven).unwrap().burn_total=2.0;factory_steps(g,20);assert!(g.world.containers[&oven].slots[2].is_none());});
    assert_eq!(g.world.containers[&bank],before);
});}
#[test]
fn factory_solar_charges_reusable_cells_only_with_visible_daylight() {with_packs(||{
    let mut g=game();let p=macroquad::math::IVec3::new(4,90,4);place(&mut g,4,90,4,"ad_absurdum:solar_panel");stock(&mut g,p,0,"ad_absurdum:empty_battery",1);
    g.time=0.75;factory_steps(&mut g,20);assert_eq!(g.world.containers[&p].burn,0.0);
    g.time=0.25;g.world.set(4,91,4,STONE);factory_steps(&mut g,20);assert_eq!(g.world.containers[&p].burn,0.0);
    g.world.set(4,91,4,AIR);factory_steps(&mut g,84);assert_eq!(g.world.containers[&p].slots[2],Some((id("ad_absurdum:battery"),1)));
    let c=&g.world.containers[&p];assert!((c.burn-6.0).abs()<0.1,"{}",c.burn);
    place(&mut g,5,90,4,"ad_absurdum:cable");place(&mut g,6,90,4,"ad_absurdum:power_bank");factory_steps(&mut g,4);
    assert!((g.world.containers[&macroquad::math::IVec3::new(6,90,4)].burn-12.0).abs()<0.1);assert_eq!(g.world.containers[&p].burn,0.0);
    let msg=crate::net::Msg::FactoryStates(g.factory_snapshot());assert_eq!(crate::net::Msg::decode(&msg.encode()).unwrap(),msg);
    let mut v=g.factory_visual(p).unwrap();v.energy=1001;assert!(crate::net::Msg::decode(&crate::net::Msg::FactoryStates(vec![v]).encode()).is_err());
});}

#[test]
fn native_factory_recipes_work_when_each_industrial_pack_is_installed_alone() {
    for pack in ["half_baked","ad_absurdum"] {
        let src=sources().into_iter().find(|s|s.id==pack).unwrap();let text=std::str::from_utf8(&src.files["mod.txt"]).unwrap();
        mods::with_mods(&[(pack,text)],|_|{
            let mut g=Game::new_with(811,true,false,GenOptions{structures:0,..GenOptions::DEFAULT});g.world.load_now(0,0);g.player.body.pos=Vec3::new(4.5,90.0,4.5);let p=macroquad::math::IVec3::new(4,90,4);
            let (machine,input,second,cell,result)=if pack=="half_baked"{("half_baked:centrifuge","half_baked:glowium_dust","coal","half_baked:energy_cell","half_baked:pellet")}else{("ad_absurdum:refinery","coal","ad_absurdum:tank","ad_absurdum:battery","ad_absurdum:fuel")};
            place(&mut g,4,90,4,machine);stock(&mut g,p,0,input,2);stock(&mut g,p,1,second,1);stock(&mut g,p,4,if pack=="half_baked"{"coal"}else{cell},1);g.open_machine(p);g.factory_action(0);factory_steps(&mut g,16);
            assert_eq!(block(id(machine)).model,Model::Shaped);assert!(crate::models::pieces(id(machine)).is_some());
            assert_eq!(g.world.containers[&p].slots[2].unwrap().0,id(result));if pack=="half_baked"{assert!((g.world.containers[&p].burn-48.0).abs()<0.001);assert!(g.world.containers[&p].slots[5].is_none());}else{assert_eq!(g.world.containers[&p].slots[5].unwrap().1,1);}
            // Older placed machines had no container record: replay creates an empty one.
            g.world.containers.remove(&p);g.world.chunks.remove(&(0,0));g.world.load_now(0,0);assert_eq!(g.world.containers[&p].slots.len(),crate::factory::SLOTS);
        });
    }
}

#[test]
fn factory_cell_swaps_cannot_convert_partial_low_capacity_work_into_extra_energy() {with_packs(||{
    let mut g=game();let p=macroquad::math::IVec3::new(4,90,4);place(&mut g,4,90,4,"ad_absurdum:power_bank");
    stock(&mut g,p,0,"ad_absurdum:empty_battery",1);g.world.containers.get_mut(&p).unwrap().burn=240.0;factory_steps(&mut g,15);
    assert!((g.world.containers[&p].cook-3.75).abs()<0.01);g.open_container(p);g.inv.cursor=None;g.container_click(0,false,false);
    assert_eq!(g.world.containers[&p].cook,0.0);g.inv.cursor=Some((id("half_baked:empty_cell"),1));g.inv.cursor_wear=0;g.container_click(0,false,false);factory_steps(&mut g,1);assert!(g.world.containers[&p].slots[2].is_none());
    // The server path also clears work when an input is withdrawn or replaced.
    g.peers.insert(77,crate::multiplayer::Peer::new("Loader".into(),g.player.body.pos));g.host_open(77,p);g.world.containers.get_mut(&p).unwrap().cook=3.75;
    g.host_container_move(77,p,0,id("half_baked:empty_cell"),1,false,0);assert_eq!(g.world.containers[&p].cook,0.0);
});}
