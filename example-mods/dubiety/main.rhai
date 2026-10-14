// An original embedded trigram corpus, not a downloaded neural model.
// The two previous words choose a weighted next word. Topic buckets keep
// the tiny model useful; the generator has a hard 28-word limit.
fn corpus() {
    #{
        general:[
            "I can help with small plans and local observations .",
            "I can remember a note if you ask me to remember .",
            "A small plan is easier to finish than a very large promise .",
            "I think the next useful step is to check your supplies .",
            "I think you can explore a little and return before night .",
            "We can build a safe place and improve it one block at a time .",
            "We can check the situation before choosing the next step .",
            "My thoughts are small but I will try to keep the words in order .",
            "My thoughts sometimes wander but your world stays yours .",
            "The useful answer depends on what you have and where you are ."
        ],
        mining:[
            "For mining bring a pickaxe and enough torches .",
            "For mining leave a path back to the surface .",
            "For mining check the block before spending a valuable tool .",
            "A safe tunnel has light and a clear way home .",
            "A safe tunnel is better than a hole full of surprises .",
            "Dig carefully near water and keep a spare pickaxe .",
            "Dig carefully near lava and do not stand on the block you remove .",
            "Iron is useful for tools and machines before fancy materials ."
        ],
        food:[
            "For food cook meat or grow a small field .",
            "For food carry a meal before a long trip .",
            "A useful farm has water and room for crops .",
            "A useful farm can feed you while you build .",
            "Eat something before a fight and keep food for the return .",
            "Cooking is a good way to make your supplies last .",
            "Low hunger means it is time to find a meal ."
        ],
        building:[
            "For building start with a floor and leave room for a door .",
            "For building use light before decorating the roof .",
            "A small house can become a larger house later .",
            "A small house needs storage and a safe entrance .",
            "Keep important machines away from places that explode .",
            "Keep a little space between the water and your workbench .",
            "A beach house looks pleasant beside grass and shells ."
        ],
        combat:[
            "For combat keep food and a clear escape route .",
            "For combat check your armour before meeting a boss .",
            "A shield can help while you decide where to move .",
            "A shield is useful when arrows are arriving .",
            "Move away when a creature prepares a large attack .",
            "Move carefully and fight one problem at a time .",
            "Low health means retreat is a sensible plan ."
        ],
        travel:[
            "For travel remember your departure point .",
            "For travel pack supplies for the return journey .",
            "A portal leads to another world with its own terrain .",
            "A portal should have a safe place on both sides .",
            "Exploration is easier with food and a way home .",
            "Exploration is easier when you leave a few landmarks ."
        ],
        nuclear:[
            "For reactors bring coolant and check the cooling tower .",
            "For reactors stop the batch before removing the machine .",
            "A nuke needs distance and a very deliberate arming key .",
            "A nuke countdown can be cancelled before the blast .",
            "Wear the full hazmat suit when the counter becomes busy .",
            "Fallout is a reason to leave and return later ."
        ],
        space:[
            "For space bring oxygen and fuel for the return trip .",
            "For space wear your helmet and carry the life support pack .",
            "A rocket needs a launch pad and a suitable tier .",
            "A rocket should be checked before spending fuel .",
            "The Moon is a useful first stop for better rocket materials .",
            "Hot planets need a heat shield as well as oxygen .",
            "Your oxygen canister is not an unlimited atmosphere ."
        ],
        dusk:[
            "For the forest bring food and remember your portal .",
            "For the forest find a courtyard before challenging the serpent .",
            "A guardian trophy opens the next challenge in your journal .",
            "A guardian is easier to fight with a friend and a plan .",
            "Moonmetal is useful for stronger tools in the dim forest .",
            "Fireflies are small lamps with very little paperwork ."
        ]
    }
}
fn on_load() {
    let model=#{};let starts=#{};let words=0;
    for topic in corpus().keys() {
        let beginnings=[];
        for sentence in corpus()[topic] {
            let tokens=sentence.split(" ");beginnings.push([tokens[0],tokens[1]]);
            for i in 2..tokens.len() {
                let key=`${topic}|${tokens[i-2]}|${tokens[i-1]}`;
                if !model.contains(key) {model[key]=[];}
                let next=model[key];next.push(tokens[i]);model[key]=next;words+=1;
            }
        }
        starts[topic]=beginnings;
    }
    set_var("model",model);set_var("starts",starts);set_var("word_count",words);
    log(`Dubiety loaded: ${model.len()} two-word states, ${words} weighted transitions, no network and no neural weights. /dubiety help`);
}
fn value(key,fallback) {let v=get_var(key);if v==() {fallback}else{v}}
fn topic(text) {
    let text=text.to_lower();
    for pair in [["space",["rocket","planet","oxygen","moon","space"]],["nuclear",["nuke","reactor","radiation","coolant"]],["dusk",["dusk","forest","guardian","serpent"]],["mining",["mine","mining","ore","pickaxe","dig"]],["food",["food","hungry","farm","cook","eat"]],["combat",["fight","combat","health","boss","shield"]],["building",["build","house","beach","roof"]],["travel",["travel","portal","home","explore"]]] {
        for word in pair[1] {if text.contains(word) {return pair[0];}}
    }
    "general"
}
fn generate(bucket) {
    let model=get_var("model");let starts=get_var("starts");let choices=starts[bucket];let seed=choices[random_int(0,choices.len()-1)];
    let a=seed[0];let b=seed[1];let text=`${a} ${b}`;
    for i in 0..26 {
        let key=`${bucket}|${a}|${b}`;if !model.contains(key) {break;}
        let choices=model[key];let next=choices[random_int(0,choices.len()-1)];if next=="." {break;}
        text+=` ${next}`;a=b;b=next;
    }
    text+"."
}
fn available(player) {
    if count_item(player,"dubiety:beacon")>0 {return true;}
    let p=player_pos(player);if p==() {return false;}
    let d=player_dimension(player);
    for y in -1..2 {for z in -2..3 {for x in -2..3 {if get_block_in(d,p[0]+x,p[1]+y,p[2]+z)=="dubiety:terminal" {return true;}}}}
    false
}
fn tail_text(args) {
    let text="";for i in 2..args.len() {if i>2 {text+=" ";}text+=args[i];}text
}
fn on_chat(player,text) {
    if text!="/dubiety" && !text.starts_with("/dubiety ") {return true;}
    let args=text.split(" ");let cmd=if args.len()>1 {args[1]}else{"help"};
    if cmd=="help" {
        message(player,"/dubiety ask <words>: generated local reply. Carry a Companion Beacon or stand beside a Terminal.");
        message(player,"/dubiety remember <note> /dubiety recall /dubiety forget. Up to eight notes, 160 characters each, saved per player in this world.");
        message(player,"/dubiety status /dubiety summon. A small word model with game-state hints; friendly, imperfect, and never secretly edits your world.");return false;
    }
    if !available(player) {message(player,"Carry a Dubiety Beacon or stand near a Dubiety Terminal.");return false;}
    switch cmd {
        "ask" => {
            let prompt=if args.len()>2 {tail_text(args)}else{"hello"};let bucket=topic(prompt);
            let recent=value(`recent_${player}`,[]);recent.push(prompt.sub_string(0,160));if recent.len()>4 {recent.remove(0);}set_var(`recent_${player}`,recent);
            score_add("dubiety_chats",player,1);message(player,`Dubiety: ${generate(bucket)}`);
            if player_health(player)<8 {message(player,"Dubiety observes: your health is low. Find somewhere safe.");}
            if player_food(player)<6 {message(player,"Dubiety observes: you need food soon.");}
        }
        "remember" => {
            if args.len()<3 {message(player,"Give me a short note after remember.");return false;}
            let note=tail_text(args).sub_string(0,160);let notes=value(`notes_${player}`,[]);if notes.len()>=8 {notes.remove(0);}notes.push(note);set_var(`notes_${player}`,notes);message(player,`Dubiety remembers: ${note}`);
        }
        "recall" => {let notes=value(`notes_${player}`,[]);if notes.len()==0 {message(player,"Dubiety: no saved notes yet.");}else{for note in notes {message(player,`Dubiety's note: ${note}`);}}}
        "forget" => {set_var(`notes_${player}`,[]);set_var(`recent_${player}`,[]);message(player,"Dubiety: your notes and recent prompts are cleared.");}
        "status" => {message(player,`Dubiety observes: ${player_dimension(player)}; health ${player_health(player)}; food ${player_food(player)}; ${player_items(player).len()} inventory entries.`);message(player,`Model: ${get_var("model").len()} states, ${get_var("word_count")} transitions. Your chats: ${score_get("dubiety_chats",player)}.`);}
        "summon" => {
            let p=player_pos(player);let d=player_dimension(player);
            if value(`summon_${player}`,0)>0 || mob_count_in(d)>=40 || mobs_near_in(d,p[0],p[1],p[2],32)>=8 {message(player,"Give the nearby creatures some room, or wait thirty seconds.");}
            else if count_item(player,"dubiety:chip")<1 {message(player,"Summoning uses one Thinking Chip; taming uses another.");}
            else {take(player,"dubiety:chip",1);spawn_mob_in(d,"dubiety:companion",p[0]+2,p[1]+1,p[2]);set_var(`summon_${player}`,30);message(player,"Dubiety has arrived. Offer a Thinking Chip to make it follow you.");}
        }
        _ => {message(player,"Dubiety: I do not know that command. Try /dubiety help.");}
    }
    false
}
fn on_tick(dt) {
    let acc=value("acc",0.0)+dt;if acc<1.0 {set_var("acc",acc);return;}set_var("acc",acc-1.0);
    for player in players() {
        set_var(`summon_${player}`,max(0,value(`summon_${player}`,0)-1));
        let cooldown=max(0,value(`hint_${player}`,0)-1);set_var(`hint_${player}`,cooldown);
        if cooldown==0 && count_item(player,"dubiety:beacon")>0 {
            if player_health(player)<6 {message(player,`Dubiety: ${generate("combat")}`);set_var(`hint_${player}`,45);}
            else if player_food(player)<5 {message(player,`Dubiety: ${generate("food")}`);set_var(`hint_${player}`,45);}
        }
    }
}
