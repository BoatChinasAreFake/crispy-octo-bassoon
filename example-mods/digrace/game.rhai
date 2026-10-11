// Dig Race: an example minigame for Minceraft.
// Everyone is split into two teams, Reds and Blues. For two minutes, every
// block you dig scores a point for you and for your team. The sidebar shows
// the scores, a countdown shows the time left, and when it's up the winners
// get a cake. Players can't hurt each other while it's on.

fn on_chat(player, text) {
    if text == "/digrace" {
        if timer_left("race") > 0.0 {
            message(player, "A race is already on!");
            return false;
        }
        start_race();
        return false;
    }
    if text == "/digrace stop" {
        timer_stop("race");
        end_race();
        return false;
    }
}

fn start_race() {
    team_create("Reds", "red");
    team_create("Blues", "blue");
    let everyone = players();
    let i = 0;
    for p in everyone {
        team_join(p, if i % 2 == 0 { "Reds" } else { "Blues" });
        i += 1;
    }
    set_pvp(false);
    score_reset("dug");
    score_reset("team");
    score_set("team", "Reds", 0);
    score_set("team", "Blues", 0);
    sidebar_show("dug", "Dig Race");
    timer_start("race", 120);
    countdown_show("race", "Dig Race ends in");
    // Everyone comes back by the start if they fall in a hole.
    if everyone.len() > 0 {
        let p = player_pos(everyone[0]);
        for q in everyone { set_spawn(q, p[0], p[1], p[2]); }
    }
    broadcast("Dig Race! Two minutes. Every block you dig is a point. Go!");
    set_var("racing", true);
}

fn on_block_break(player, x, y, z, block) {
    if get_var("racing") != true { return; }
    score_add("dug", player, 1);
    let t = team_of(player);
    if t != "" { score_add("team", t, 1); }
}

fn on_timer(name) {
    if name == "race" { end_race(); }
    // A little after the end, the scores go away.
    if name == "tidy" { sidebar_hide(); }
}

fn end_race() {
    if get_var("racing") != true { return; }
    set_var("racing", false);
    countdown_hide();
    let reds = score_get("team", "Reds");
    let blues = score_get("team", "Blues");
    let winners = if reds > blues { "Reds" } else if blues > reds { "Blues" } else { "" };
    if winners == "" {
        broadcast(`It's a draw, ${reds} each! Everyone gets a cake anyway.`);
        for p in players() { give(p, "cake"); }
    } else {
        broadcast(`${winners} win, ${reds} to ${blues}! Cake for the winners.`);
        for p in team_members(winners) { give(p, "cake"); }
    }
    sidebar_show("team", "Final score");
    timer_start("tidy", 15);
}
