// Fictional voxel machinery. No real nuclear engineering is represented.
fn number(key) { let n=get_var(key); if n==() {0} else {n} }
fn near(player,block) {
    let p=player_pos(player);if p==() {return ();}
    let d=player_dimension(player);
    for y in -2..3 {for z in -3..4 {for x in -3..4 {
        let at=[p[0].floor()+x,p[1].floor()+y,p[2].floor()+z];
        if get_block_in(d,at[0],at[1],at[2])==`half_baked:${block}` {return at;}
    }}}
    ()
}
fn pay(player,inputs) {
    for pair in inputs {if count_item(player,pair[0])<pair[1] {message(player,`Need ${pair[1]} ${pair[0]}.`);return false;}}
    for pair in inputs {take(player,pair[0],pair[1]);} true
}
fn suit(player) {
    wearing(player,"half_baked:hazmat_hood") && wearing(player,"half_baked:hazmat_jacket") &&
    wearing(player,"half_baked:hazmat_trousers") && wearing(player,"half_baked:hazmat_boots")
}
fn shielded(d,source,target) {
    for i in 1..9 {
        let fraction=i.to_float()/9.0;
        if get_block_in(d,source[0]+(target[0]-source[0])*fraction,source[1]+(target[1]-source[1])*fraction,source[2]+(target[2]-source[2])*fraction)=="half_baked:shielding" {return true;}
    }
    false
}
fn on_load() {
    for key in ["reactors","bombs","zones"] {if get_var(key)==() {set_var(key,[]);}}
    log("Half-Baked Nuclear Machinery: /hbm help. Unarmed nukes are safe to mine.");
}
fn on_chat(player,text) {
    if text!="/hbm" && !text.starts_with("/hbm ") {return true;}
    let args=text.split(" ");let cmd=if args.len()>1 {args[1]} else {"help"};
    switch cmd {
        "help" => {
            message(player,"Load machine input slots and power, then use its panel or /hbm refine. /hbm reactor runs stocked fuel + coolant with a cooling tower.");
            message(player,"/hbm status /hbm scram /hbm geiger /hbm decon /hbm tablet. Wear all four hazmat pieces near fallout.");
            message(player,"/hbm arm small|large near a placed nuke, with an arming key: 10s fuse. /hbm disarm during the fuse. Big blasts leave 120s fallout.");
        }
        "refine" => {
            if near(player,"centrifuge")==() {message(player,"Stand within three blocks of a Spin Doctor Centrifuge.");}
            else if pay(player,[["half_baked:glowium_dust",2],["coal",1]]) {give(player,"half_baked:pellet",1);score_add("hbm_refined",player,1);message(player,"Pellet refined. It is still not a snack.");}
        }
        "reactor" => {
            let at=near(player,"reactor");let jobs=get_var("reactors");let d=player_dimension(player);
            if at==() {message(player,"Stand beside a Questionable Reactor.");return false;}
            for job in jobs {if job.dim==d && job.at==at {message(player,"That reactor already has a batch.");return false;}}
            if jobs.len()>=16 {message(player,"All sixteen reactor slots are busy.");return false;}
            let tower=near(player,"cooling_tower");
            if tower==() {message(player,"A cooling tower must be within three blocks too.");return false;}
            if pay(player,[["half_baked:pellet",1],["half_baked:coolant",1]]) {
                jobs.push(#{owner:player,dim:d,at:at,tower:tower,left:15});set_var("reactors",jobs);
                message(player,"Coolant circulation started. Batch ready in fifteen seconds.");
            }
        }
        "scram" => {
            let at=near(player,"reactor");let keep=[];let found=false;
            for job in get_var("reactors") {if job.dim==player_dimension(player) && job.at==at && job.owner==player {found=true;} else {keep.push(job);}}
            set_var("reactors",keep);message(player,if found {"Emergency stop. The spent fuel and coolant are lost."} else {"No reactor batch owned by you nearby."});
        }
        "status" => {message(player,`Refined pellets: ${score_get("hbm_refined",player)}. Completed reactor batches: ${score_get("hbm_batches",player)}.`);}
        "arm" => {
            let kind=if args.len()>2 {args[2]} else {"small"};
            if kind!="small" && kind!="large" {message(player,"Use /hbm arm small or /hbm arm large.");return false;}
            let block=if kind=="small" {"small_nuke"} else {"large_nuke"};let at=near(player,block);let bombs=get_var("bombs");let d=player_dimension(player);
            if at==() {message(player,"Place the matching nuke and stand within three blocks.");return false;}
            for b in bombs {if b.dim==d && b.at==at {message(player,"Already armed. Use /hbm disarm.");return false;}}
            if bombs.len()>=8 {message(player,"Eight countdowns are already running.");return false;}
            if pay(player,[["half_baked:arming_key",1]]) {
                bombs.push(#{owner:player,dim:d,at:at,block:block,big:kind=="large",left:10});set_var("bombs",bombs);
                broadcast(`${player} armed ${kind} Half-Baked nuke in ${d}. Ten seconds. Move away!`);
            }
        }
        "disarm" => {
            let keep=[];let p=player_pos(player);let found=false;
            for b in get_var("bombs") {
                if b.owner==player && b.dim==player_dimension(player) && abs(b.at[0]-p[0])<=4 && abs(b.at[2]-p[2])<=4 && abs(b.at[1]-p[1])<=4 {found=true;give(player,"half_baked:arming_key",1);} else {keep.push(b);}
            }
            set_var("bombs",keep);message(player,if found {"Countdown cancelled."} else {"No countdown owned by you nearby."});
        }
        "geiger" => {
            if count_item(player,"half_baked:geiger")<1 {message(player,"Carry a Click Counter first.");}
            else {message(player,`Radiation dose: ${number(`dose_${player}`)}. Suit protection: ${suit(player)}.`);}
        }
        "decon" => {
            if count_item(player,"half_baked:scrubber")<1 {message(player,"Carry a Portable Scrubber.");}
            else if pay(player,[["half_baked:coolant",1]]) {set_var(`dose_${player}`,0);message(player,"Dose cleared. Nearby fallout can still expose you again.");}
        }
        "tablet" => {if pay(player,[["half_baked:tablet",1]]) {set_var(`dose_${player}`,max(0,number(`dose_${player}`)-15));}}
        _ => {message(player,"Unknown Half-Baked command. /hbm help");}
    }
    false
}
fn on_tick(dt) {
    let acc=number("acc")+dt;if acc<1.0 {set_var("acc",acc);return;}set_var("acc",acc-1.0);
    let jobs=[];
    for job in get_var("reactors") {
        if !chunk_loaded_in(job.dim,job.at[0],job.at[2]) || !chunk_loaded_in(job.dim,job.tower[0],job.tower[2]) {jobs.push(job);continue;}
        if get_block_in(job.dim,job.at[0],job.at[1],job.at[2])=="half_baked:reactor" {
            if get_block_in(job.dim,job.tower[0],job.tower[1],job.tower[2])!="half_baked:cooling_tower" {
                explode_in(job.dim,job.at[0],job.at[1],job.at[2],3);
                let zones=get_var("zones");if zones.len()>=64 {zones.remove(0);}zones.push(#{dim:job.dim,at:job.at,radius:12,left:60});set_var("zones",zones);
                message(job.owner,"Cooling tower removed during operation. Reactor meltdown.");continue;
            }
            job.left-=1;
            if job.left<=0 {give(job.owner,"half_baked:energy_cell",4);give(job.owner,"half_baked:waste",1);score_add("hbm_batches",job.owner,1);message(job.owner,"Reactor batch completed: four energy cells and one spent nonsense.");}
            else {jobs.push(job);}
        } else {message(job.owner,"Reactor was removed or is unloaded; batch stopped.");}
    }
    set_var("reactors",jobs);
    let bombs=[];let zones=get_var("zones");
    for b in get_var("bombs") {
        if !chunk_loaded_in(b.dim,b.at[0],b.at[2]) {bombs.push(b);continue;}
        // Mining an armed device cancels its fuse, even after a restart.
        if get_block_in(b.dim,b.at[0],b.at[1],b.at[2])!=`half_baked:${b.block}` {continue;}
        b.left-=1;
        if b.left==5 {broadcast(`Half-Baked countdown: five seconds in ${b.dim}.`);}
        if b.left<=0 {
            let at=b.at;set_block_in(b.dim,at[0],at[1],at[2],"air");
            if b.big {
                // A bounded fictional cluster blast; no real weapon simulation.
                for dz in [-8,0,8] {for dx in [-8,0,8] {explode_in(b.dim,at[0]+dx,at[1],at[2]+dz,8);}}
            } else {explode_in(b.dim,at[0],at[1],at[2],6);}
            if zones.len()>=64 {zones.remove(0);}
            zones.push(#{dim:b.dim,at:at,radius:if b.big {30}else{18},left:120});
            score_add("hbm_detonations",b.owner,1);broadcast(`Half-Baked nuke detonated in ${b.dim}. Fallout lasts two minutes.`);
        } else {bombs.push(b);}
    }
    set_var("bombs",bombs);let fresh=[];for zone in zones {zone.left-=1;if zone.left>0 {fresh.push(zone);}}set_var("zones",fresh);
    for player in players() {
        let p=player_pos(player);let d=player_dimension(player);let spicy=false;
        for zone in fresh {if zone.dim==d && abs(zone.at[0]-p[0])<zone.radius && abs(zone.at[2]-p[2])<zone.radius && abs(zone.at[1]-p[1])<zone.radius && !shielded(d,zone.at,p) {spicy=true;}}
        if count_item(player,"half_baked:waste")>0 || count_item(player,"half_baked:glowium_dust")>8 {spicy=true;}
        let dose=max(0,number(`dose_${player}`)+if spicy && !suit(player) {2}else{-1});set_var(`dose_${player}`,dose);
        if dose>30 && dose%5==0 {damage(player,1);message(player,"The Click Counter would like you to leave this area.");}
    }
}
