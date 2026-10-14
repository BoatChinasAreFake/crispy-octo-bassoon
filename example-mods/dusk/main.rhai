fn value(key,fallback) {let v=get_var(key);if v==() {fallback}else{v}}
fn altar(player,kind) {
    let p=player_pos(player);if p==() {return ();}
    let altars=#{serpent:"serpent_altar",lich:"lich_altar",minotax:"minotaur_altar",hydra:"hydra_altar"};
    for y in -2..3 {for z in -4..5 {for x in -4..5 {
        let at=[p[0].floor()+x,p[1].floor()+y,p[2].floor()+z];
        if get_block_in("dusk:twilit_realm",at[0],at[1],at[2])==`dusk:${altars[kind]}` {return at;}
    }}}
    ()
}
fn challenge(player,kind,at) {
    let order=["serpent","lich","minotax","hydra"];let stage=order.index_of(kind);
    if player_dimension(player)!="dusk:twilit_realm" || stage<0 {message(player,"Challenges belong to the Slightly Dim Forest.");return;}
    if score_get("dusk_progress",player)<stage {message(player,"Defeat the earlier guardian first: serpent, lich, minotax, then hydra.");return;}
    if at==() {message(player,"Stand within four blocks of that guardian's altar.");return;}
    let fights=value("fights",[]);for f in fights {if f.kind==kind {message(player,"That guardian is already fighting. Join the encounter or wait.");return;}}
    if mob_count_in("dusk:twilit_realm")>=32 {message(player,"Too many creatures here for another guardian.");return;}
    spawn_mob_in("dusk:twilit_realm",`dusk:${kind}`,at[0],at[1]+1,at[2]);
    fights.push(#{kind:kind,owner:player,at:at,idle:0});set_var("fights",fights);
    broadcast(`${player} challenged the ${kind} in the Slightly Dim Forest.`);
}
fn on_load() {if get_var("fights")==() {set_var("fights",[]);}log("Slightly Dim Forest: build a Duskwood frame and light with a Sparker. /dusk help");}
fn on_chat(player,text) {
    if text!="/dusk" && !text.starts_with("/dusk ") {return true;}
    let args=text.split(" ");let cmd=if args.len()>1 {args[1]}else{"help"};
    switch cmd {
        "help" => {
            message(player,"Craft Duskwood Gate Frames; build and spark a normal portal frame. Everything wild in this mod stays in dusk:twilit_realm.");
            message(player,"/dusk where serpent|lich|minotax|hydra finds a guardian structure. /dusk challenge <guardian> beside its altar begins the battle.");
            message(player,"Order: Soggy Serpent -> Overdue Librarian -> Minotax -> Complaint Department. Co-op helps; owner and finisher both gain progress.");
            message(player,"/dusk journal shows trophies. Mine Moonmetal, gather Duskberries and craft the Moonmetal tools. Step through a portal to return.");
        }
        "journal" => {let n=score_get("dusk_progress",player);message(player,`Guardians completed: ${n}/4. Moonmetal and trophy items stay with you between worlds.`);}
        "where" => {
            if player_dimension(player)!="dusk:twilit_realm" {message(player,"Enter the forest before surveying its ruins.");return false;}
            let kind=if args.len()>2 {args[2]}else{"serpent"};let keys=#{serpent:"serpent_court",lich:"lich_library",minotax:"minotax_maze",hydra:"hydra_mound"};
            if !keys.contains(kind) {message(player,"Choose serpent, lich, minotax or hydra.");return false;}
            let p=player_pos(player);let at=locate_structure_in("dusk:twilit_realm",`dusk:${keys[kind]}`,p[0],p[2]);
            message(player,if at==() {"No such ruin found within this survey."}else{`Nearest ${kind} ruin: ${at[0]}, ${at[1]}, ${at[2]}.`});
        }
        "challenge" => {
            let kind=if args.len()>2 {args[2]}else{"serpent"};
            if !["serpent","lich","minotax","hydra"].contains(kind) {message(player,"Choose serpent, lich, minotax or hydra.");return false;}
            if player_dimension(player)!="dusk:twilit_realm" {message(player,"Enter the forest first.");return false;}
            challenge(player,kind,altar(player,kind));
        }
        _ => {message(player,"Unknown dusk command. /dusk help");}
    }
    false
}
fn on_block_break(player,x,y,z,block) {
    if dimension()!="dusk:twilit_realm" {return true;}
    let keys=#{"dusk:serpent_altar":"serpent","dusk:lich_altar":"lich","dusk:minotaur_altar":"minotax","dusk:hydra_altar":"hydra"};
    if keys.contains(block) {challenge(player,keys[block],[x,y,z]);return false;}
    true
}
fn on_mob_death(kind,x,y,z,killer) {
    if dimension()!="dusk:twilit_realm" {return;}
    let stages=#{"dusk:serpent":1,"dusk:lich":2,"dusk:minotax":3,"dusk:hydra":4};
    if !stages.contains(kind) {return;}
    let stage=stages[kind];let plain=kind.split(":")[1];let keep=[];let owner="";
    for f in value("fights",[]) {if f.kind==plain {owner=f.owner;}else{keep.push(f);}}
    set_var("fights",keep);
    if killer!="" {score_set("dusk_progress",killer,max(stage,score_get("dusk_progress",killer)));message(killer,`Guardian defeated. Journal: ${stage}/4.`);}
    if owner!="" && killer!="" && owner!=killer {score_set("dusk_progress",owner,max(stage,score_get("dusk_progress",owner)));message(owner,"Your co-op guardian encounter is complete.");}
}
fn on_tick(dt) {
    let acc=value("acc",0.0)+dt;if acc<5.0 {set_var("acc",acc);return;}set_var("acc",acc-5.0);
    // A stale fight only clears when its loaded encounter region is empty.
    // Bosses are saved by the engine; an unloaded realm never resets its lock.
    let fights=[];for f in value("fights",[]) {
        if chunk_loaded_in("dusk:twilit_realm",f.at[0],f.at[2]) && mobs_near_in("dusk:twilit_realm",f.at[0],f.at[1],f.at[2],64)==0 {f.idle+=5;}else{f.idle=0;}
        if f.idle<60 {fights.push(f);}
    }set_var("fights",fights);
    let pending=0;
    for player in players() {
        if player_dimension(player)!="dusk:twilit_realm" {continue;}
        let p=player_pos(player);if mob_count_in("dusk:twilit_realm")+pending>=24 || mobs_near_in("dusk:twilit_realm",p[0],p[1],p[2],48)>=8 {continue;}
        let x=p[0].floor()+random_int(-18,18);let z=p[2].floor()+random_int(-18,18);let y=surface_y_in("dusk:twilit_realm",x,z)+1;
        let kind=["deer","firefly_mob","kobold","thornling"][random_int(0,3)];
        let ground=get_block_in("dusk:twilit_realm",x,y-1,z);
        if kind!="firefly_mob" && !["dusk:dusk_grass","dusk:dusk_soil","dusk:dusk_stone","dusk:rune_brick"].contains(ground) {continue;}
        spawn_mob_in("dusk:twilit_realm",`dusk:${kind}`,x,y,z);pending+=1;
    }
}
