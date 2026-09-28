// Handy Commands: an example code mod for Minceraft.
// Scripts run where the world lives (single player, the host, or a dedicated
// server), so these commands work for everyone in multiplayer too.
// Language: Rhai (https://rhai.rs/book). Game API: SCRIPTING.md.

fn on_load() {
    log("Handy Commands loaded. Type /help in chat.");
}

fn on_player_join(player) {
    broadcast(`Welcome, ${player}! Type /help for commands.`);
}

// Chat commands. Returning false means "handled, don't post it as chat".
fn on_chat(player, text) {
    if !text.starts_with("/") { return; }
    let words = text.split(" ");
    let cmd = words[0];

    switch cmd {
        "/help" => {
            message(player, "/home /sethome - save and return to a spot");
            message(player, "/tower [height] - glass tower next to you");
            message(player, "/day /night /heal /jump /kit");
            message(player, "/spawn <oinker|hisser|groaner>  /who");
        }
        "/sethome" => {
            let p = player_pos(player);
            set_var(`home_${player}`, p);
            message(player, `Home set at ${p[0].floor()}, ${p[1].floor()}, ${p[2].floor()}.`);
        }
        "/home" => {
            let home = get_var(`home_${player}`);
            if home == () {
                message(player, "No home yet. Use /sethome first.");
            } else {
                teleport(player, home[0], home[1], home[2]);
                play_sound("pop", home[0], home[1], home[2]);
                message(player, "Welcome home.");
            }
        }
        "/tower" => {
            let height = if words.len() > 1 { parse_int(words[1]) } else { 8 };
            if height < 1 || height > 60 { height = 8; }
            let p = player_pos(player);
            let x = p[0].floor() + 2;
            let y = p[1].floor();
            let z = p[2].floor();
            fill(x, y, z, x, y + height - 1, z, "glass");
            set_block(x, y + height, z, "glowrock");
            message(player, `Built a ${height}-block tower. Very tall. Very glass.`);
        }
        "/day" => { set_time(0.05); broadcast(`${player} made it day.`); }
        "/night" => { set_time(0.55); broadcast(`${player} summoned the night. Thanks, ${player}.`); }
        "/heal" => { heal(player, 20); message(player, "Fully healed."); }
        "/jump" => { launch(player, 25); }
        "/kit" => {
            if get_var(`kit_${player}`) == true {
                message(player, "You already took your starter kit.");
            } else {
                set_var(`kit_${player}`, true);
                give(player, "stone_pickaxe", 1);
                give(player, "torch", 16);
                give(player, "porkchop", 5);
                message(player, "Starter kit delivered. Use it wisely.");
            }
        }
        "/spawn" => {
            let kind = if words.len() > 1 { words[1] } else { "oinker" };
            let p = player_pos(player);
            spawn_mob(kind, p[0] + 2.0, p[1] + 1.0, p[2]);
        }
        "/who" => {
            let names = players();
            message(player, `${names.len()} online: ${names}`);
        }
        _ => { return true; }  // not ours: let the game say "Unknown command"
    }
    false
}

// A small reward for mining: 1 in 100 stone blocks hides a Dimond.
fn on_block_break(player, x, y, z, block) {
    if block == "stone" && random() < 0.01 {
        give(player, "diamond", 1);
        message(player, "You found a Dimond inside the stone! Lucky.");
    }
    // Returning nothing lets the block break normally.
}

// Keep a kill counter per player and cheer every 10 kills.
fn on_mob_death(kind, x, y, z, killer) {
    if killer == "" { return; }
    let key = `kills_${killer}`;
    let kills = get_var(key);
    if kills == () { kills = 0; }
    kills += 1;
    set_var(key, kills);
    if kills % 10 == 0 {
        broadcast(`${killer} has defeated ${kills} mobs!`);
    }
}

// Runs 20 times a second. Announce nightfall once per night.
fn on_tick(dt) {
    let night = is_night();
    if night && get_var("announced") != true {
        set_var("announced", true);
        broadcast("Night falls. The Groaners are restless...");
    }
    if !night { set_var("announced", false); }
}
