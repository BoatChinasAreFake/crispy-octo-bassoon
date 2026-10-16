fn value(key,fallback) {let v=get_var(key);if v==() {fallback}else{v}}
fn nearby(player,block) {
    let p=player_pos(player);let d=player_dimension(player);
    if p==() {return ();}
    for y in -2..3 {for z in -3..4 {for x in -3..4 {
        let at=[p[0].floor()+x,p[1].floor()+y,p[2].floor()+z];
        if get_block_in(d,at[0],at[1],at[2])==`ad_absurdum:${block}` {return at;}
    }}}
    ()
}
fn pay(player,inputs) {
    for pair in inputs {if count_item(player,pair[0])<pair[1] {message(player,`Need ${pair[1]} ${pair[0]}.`);return false;}}
    for pair in inputs {take(player,pair[0],pair[1]);}true
}
fn on_load() {
    if get_var("flights")==() {set_var("flights",[]);}
    if get_var("rockets")==() {set_var("rockets",[]);}
    log("Ad Absurdum: /astra help. Five separate planets; rockets are recovered after landing.");
}
fn on_chat(player,text) {
    if text!="/astra" && !text.starts_with("/astra ") {return true;}
    let args=text.split(" ");let cmd=if args.len()>1 {args[1]}else{"help"};let p=player_pos(player);let d=player_dimension(player);
    if p==() {return false;}
    switch cmd {
        "help" => {
            message(player,"Load machine input slots and connect power. /astra refine and /astra refill start the nearby stocked machine. Solar panels charge empty batteries.");
            message(player,"Build a launch pad beside a rocket bench. /astra assemble 1|2|3|4 deploys a crafted rocket. /astra launch moon|mars|venus|mercury|glacio|home costs fuel.");
            message(player,"Tier 1 Moon; tier 2 Marzipan; tier 3 Venus/Mercurial; tier 4 Glaze-io. Wear helmet and chestplate; carry a Life Support Pack and oxygen. Hot planets need a Heat Shield.");
            message(player,"/astra status /astra rover /astra charge. Return trips need a rocket, launch pad and fuel too; there is no free return teleport.");
        }
        "refine" => {
            if nearby(player,"refinery")==() {message(player,"Stand beside a Fuel Refiner.");}
            else if pay(player,[["coal",2],["ad_absurdum:tank",1]]) {give(player,"ad_absurdum:fuel",2);message(player,"Two fuel canisters refined.");}
        }
        "refill" => {
            if nearby(player,"oxygen_generator")==() {message(player,"Stand beside a Breath Printer.");}
            else if pay(player,[["ad_absurdum:battery",1],["ad_absurdum:tank",1]]) {give(player,"ad_absurdum:oxygen",4);message(player,"Four oxygen canisters filled.");}
        }
        "charge" => {
            if nearby(player,"solar_panel")==() {message(player,"Stand beside a Sun Tax Panel.");}
            else if value(`charge_${player}`,0)>0 {message(player,"Your solar recharge needs sixty seconds between batches.");}
            else if pay(player,[["ad_absurdum:steel",1],["zap_dust",1]]) {give(player,"ad_absurdum:battery",2);set_var(`charge_${player}`,60);message(player,"Two solar batteries charged.");}
        }
        "assemble" => {
            let tier=if args.len()>2 {args[2]}else{"1"};
            if !["1","2","3","4"].contains(tier) {message(player,"Choose tier 1, 2, 3 or 4.");return false;}
            let at=nearby(player,"launch_pad");if at==() || nearby(player,"rocket_workbench")==() {message(player,"A launch pad and rocket bench must be within three blocks.");return false;}
            if at[1]>245 {message(player,"Not enough sky above this pad.");return false;}
            for y in 1..6 {if get_block_in(d,at[0],at[1]+y,at[2])!="air" {message(player,"Clear five blocks above the pad.");return false;}}
            let rockets=get_var("rockets");if rockets.len()>=64 {message(player,"Sixty-four deployed rockets are enough paperwork.");return false;}
            if pay(player,[[`ad_absurdum:rocket_${tier}`,1]]) {
                set_block_in(d,at[0],at[1]+1,at[2],"ad_absurdum:rocket_engine");
                for y in 2..5 {set_block_in(d,at[0],at[1]+y,at[2],"ad_absurdum:rocket_body");}
                set_block_in(d,at[0],at[1]+5,at[2],"ad_absurdum:rocket_nose");
                rockets.push(#{owner:player,dim:d,at:at,tier:tier});set_var("rockets",rockets);message(player,`Tier ${tier} rocket assembled. Stand beside it and choose a destination.`);
            }
        }
        "pack" => {
            let keep=[];let found=false;
            for r in get_var("rockets") {
                if r.owner==player && r.dim==d && abs(r.at[0]-p[0])<=4 && abs(r.at[2]-p[2])<=4 && abs(r.at[1]-p[1])<=6 {
                    let complete=true;for y in 1..6 {let expected=if y==1 {"rocket_engine"}else if y==5 {"rocket_nose"}else{"rocket_body"};if get_block_in(d,r.at[0],r.at[1]+y,r.at[2])!=`ad_absurdum:${expected}` {complete=false;}}
                    if complete {for y in 1..6 {set_block_in(d,r.at[0],r.at[1]+y,r.at[2],"air");}give(player,`ad_absurdum:rocket_${r.tier}`,1);found=true;}
                } else {keep.push(r);}
            }
            set_var("rockets",keep);message(player,if found {"Rocket repacked."}else{"No intact rocket owned by you nearby."});
        }
        "launch" => {
            let destination=if args.len()>2 {args[2]}else{"moon"};let tiers=#{moon:1,mars:2,venus:3,mercury:3,glacio:4,home:1};
            if !tiers.contains(destination) {message(player,"Choose moon, mars, venus, mercury, glacio or home.");return false;}
            let flights=get_var("flights");for flight in flights {if flight.owner==player {message(player,"Already in a launch sequence.");return false;}}
            if flights.len()>=16 {message(player,"Sixteen simultaneous launches is the limit.");return false;}
            let target=if destination=="home" {"overworld"}else{`ad_absurdum:${destination}`};
            if target==d {message(player,"You are already there.");return false;}
            let home=value(`home_${player}`,());if destination=="home" && home==() {message(player,"No saved departure point yet.");return false;}
            if destination!="home" && (!wearing(player,"ad_absurdum:space_helmet") || !wearing(player,"ad_absurdum:space_chestplate") || count_item(player,"ad_absurdum:life_support")<1 || count_item(player,"ad_absurdum:oxygen")<1) {message(player,"Wear helmet and chestplate; carry life support and oxygen.");return false;}
            if ["venus","mercury"].contains(destination) && count_item(player,"ad_absurdum:heat_shield")<1 {message(player,"A Heat Shield is required for that planet.");return false;}
            let chosen=();let keep=[];
            for r in get_var("rockets") {
                if chosen==() && r.owner==player && r.dim==d && abs(r.at[0]-p[0])<=4 && abs(r.at[2]-p[2])<=4 && abs(r.at[1]-p[1])<=6 {chosen=r;}else{keep.push(r);}
            }
            if chosen==() {message(player,"No deployed rocket owned by you nearby.");return false;}
            if parse_int(chosen.tier)<tiers[destination] {message(player,"This rocket tier cannot reach that world.");return false;}
            if get_block_in(d,chosen.at[0],chosen.at[1],chosen.at[2])!="ad_absurdum:launch_pad" {message(player,"The launch pad is missing.");return false;}
            for y in 1..6 {let expected=if y==1 {"rocket_engine"}else if y==5 {"rocket_nose"}else{"rocket_body"};if get_block_in(d,chosen.at[0],chosen.at[1]+y,chosen.at[2])!=`ad_absurdum:${expected}` {message(player,"The rocket is incomplete; launch cancelled.");return false;}}
            for y in 6..55 {if get_block_in(d,chosen.at[0],chosen.at[1]+y,chosen.at[2])!="air" {message(player,"Clear the vertical launch column above the rocket before launching.");return false;}}
            let bench=nearby(player,"rocket_workbench");if bench==() {message(player,"A rocket bench must be nearby for preflight checks.");return false;}
            if !pay(player,[["ad_absurdum:fuel",tiers[destination]]]) {return false;}
            if d=="overworld" {set_var(`home_${player}`,[p[0],p[2]]);}
            let landing=if destination=="home" {home}else{[p[0],p[2]]};
            flights.push(#{owner:player,origin:d,target:target,at:chosen.at,tier:chosen.tier,left:4.0,arriving:false,landing:landing});set_var("flights",flights);set_var("rockets",keep);
            for y in 1..6 {set_block_in(d,chosen.at[0],chosen.at[1]+y,chosen.at[2],"air");}
            set_block_in(d,chosen.at[0],chosen.at[1],chosen.at[2],"air");set_block_in(d,bench[0],bench[1],bench[2],"air");
            rocket_depart(player,chosen.at[0],chosen.at[1]+1,chosen.at[2],parse_int(chosen.tier),4);message(player,`Liftoff! Autopilot ascent to ${target}, followed by a powered landing. F5 switches exterior view.`);
        }
        "rover" => {if mob_count_in(d)>=40 {message(player,"Too many creatures for another rover.");} else if pay(player,[["ad_absurdum:rover",1]]) {spawn_mob_in(d,"ad_absurdum:rover",p[0]+2,p[1],p[2]);message(player,"Tame the rover with a Flight Circuit. It follows and trades batteries for supplies.");}}
        "status" => {message(player,`Location: ${d}. Oxygen buffer: ${value(`air_${player}`,0)} seconds. Worlds visited: ${score_get("astra_visits",player)}.`);}
        _ => {message(player,"Unknown space command. /astra help");}
    }
    false
}
fn on_tick(dt) {
    let flights=[];
    for flight in get_var("flights") {
        if !players().contains(flight.owner) || player_health(flight.owner)<=0 {flights.push(flight);continue;}
        let arriving=flight.contains("arriving") && flight.arriving;
        if arriving {
            if player_dimension(flight.owner)!=flight.target {rocket_clear(flight.owner);travel(flight.owner,flight.target,flight.landing[0],flight.landing[1]);rocket_land(flight.owner,parse_int(flight.tier),flight.left);flights.push(flight);continue;}
            rocket_land(flight.owner,parse_int(flight.tier),flight.left);flight.left-=min(dt,1.0);
            if flight.left<=0 {
                rocket_clear(flight.owner);
                give(flight.owner,`ad_absurdum:rocket_${flight.tier}`,1);give(flight.owner,"ad_absurdum:launch_pad",1);give(flight.owner,"ad_absurdum:rocket_workbench",1);
                let visit=`visited_${flight.owner}_${flight.target}`;if value(visit,false)!=true {score_add("astra_visits",flight.owner,1);set_var(visit,true);}
                message(flight.owner,"Touchdown complete. Rocket recovered; deploy the returned pad and bench for the next launch.");
            } else {flights.push(flight);}
        } else {
            rocket_depart(flight.owner,flight.at[0],flight.at[1]+1,flight.at[2],parse_int(flight.tier),flight.left);flight.left-=min(dt,1.0);
            if flight.left<=0 {
                rocket_clear(flight.owner);travel(flight.owner,flight.target,flight.landing[0],flight.landing[1]);
                flight.arriving=true;flight.left=3.0;rocket_land(flight.owner,parse_int(flight.tier),3);
                message(flight.owner,"Atmospheric entry. Autopilot is lowering the rocket to the landing site.");
            }
            flights.push(flight);
        }
    }
    set_var("flights",flights);
    let acc=value("acc",0.0)+dt;if acc<1.0 {set_var("acc",acc);return;}set_var("acc",acc-1.0);

    let spawn_clock=value("spawn_clock",0)+1;set_var("spawn_clock",spawn_clock%20);
    for player in players() {
        set_var(`charge_${player}`,max(0,value(`charge_${player}`,0)-1));
        let d=player_dimension(player);if !d.starts_with("ad_absurdum:") {continue;}
        let air=max(0,value(`air_${player}`,0)-1);let suited=wearing(player,"ad_absurdum:space_helmet") && wearing(player,"ad_absurdum:space_chestplate") && count_item(player,"ad_absurdum:life_support")>0;
        if suited && air<=0 && count_item(player,"ad_absurdum:oxygen")>0 {take(player,"ad_absurdum:oxygen",1);give(player,"ad_absurdum:tank",1);air=30;}
        set_var(`air_${player}`,air);
        if spawn_clock%5==0 && (!suited || air<=0) {damage(player,2);message(player,"Suit or oxygen missing. The atmosphere is not accepting visitors.");}
        if spawn_clock%5==0 && ["ad_absurdum:venus","ad_absurdum:mercury"].contains(d) && count_item(player,"ad_absurdum:heat_shield")<1 {damage(player,2);}
        if spawn_clock%20==0 {
            let p=player_pos(player);let x=p[0].floor()+12;let z=p[2].floor()+8;
            if mob_count_in(d)<40 && mobs_near_in(d,p[0],p[1],p[2],64)<8 {let y=surface_y_in(d,x,z)+1;let planet=d.split(":")[1];spawn_mob_in(d,`ad_absurdum:${planet}_crawler`,x,y,z);}
        }
    }
}
