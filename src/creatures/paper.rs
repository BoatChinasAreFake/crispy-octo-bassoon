//! **The village paper** (an original): each village's daily newspaper,
//! reporting what went on in the world yesterday.
//!
//! - Where the world lives, the day's goings-on are noted down: trees
//!   punched with bare hands, players' unfortunate ends, raids won and lost,
//!   Hissers going off near a village, new houses and Clankers, bounties done,
//!   babies, Hmmers made Master, sales at Shop Stalls, bosses beaten, storms.
//! - Each morning a fresh issue is out. Take one (free, once a day) from any
//!   village's Notice Board; hold it to read it. Every village has its own
//!   paper (The Little Hmmington Gazette, The Muddlecombe Bugle...), but the
//!   news is the same everywhere: there's only so much of it.
//! - A quiet day still makes the paper. It always does.
//!
//! A paper is an item whose wear says which day's issue it is and whose
//! masthead it carries; the stories come from the host (joined players ask).

use crate::block::*;
use crate::game::Game;
use crate::net::Msg;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Days of news kept.
const KEEP_DAYS: u32 = 8;
/// Most stories in an issue.
pub const STORIES: usize = 6;

/// Something that happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Event {
    Boss,
    RaidWon,
    RaidLost,
    Died,
    Hisser,
    Master,
    Clanker,
    House,
    TreePunched,
    Bounty,
    Sale,
    Baby,
    Storm,
}

impl Event {
    const ALL: [Event; 13] = [Event::Boss, Event::RaidWon, Event::RaidLost, Event::Died, Event::Hisser, Event::Master, Event::Clanker, Event::House, Event::TreePunched, Event::Bounty, Event::Sale, Event::Baby, Event::Storm];

    fn index(self) -> u8 {
        Event::ALL.iter().position(|e| *e == self).unwrap_or(0) as u8
    }

    fn from_index(i: u8) -> Option<Event> {
        Event::ALL.get(i as usize).copied()
    }
}

/// One thing in the news: what, who (a player, a village or a boss), any more to say, how many.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Story {
    pub event: Event,
    pub who: String,
    pub detail: String,
    pub n: u32,
}

/// The news (where the world lives), who's had today's paper, and (a joined
/// player's game) the issues the host has sent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct News {
    pub days: BTreeMap<u32, Vec<Story>>,
    pub taken: HashSet<(String, u32)>,
    /// (Not saved.) Issues to hand: day -> stories (headline, body).
    pub issues: HashMap<u32, Vec<(String, String)>>,
    /// (Not saved.) Seconds until a joined player asks again for an issue it hasn't got.
    pub ask: f32,
}

fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(120).collect()
}

impl News {
    /// Note something that happened on day `day` (the same thing again adds up).
    pub fn note(&mut self, day: u32, event: Event, who: &str, detail: &str, n: u32) {
        let list = self.days.entry(day).or_default();
        match list.iter_mut().find(|s| s.event == event && s.who == who && (s.detail == detail || event != Event::Died)) {
            Some(s) => s.n += n,
            None => list.push(Story { event, who: clean(who), detail: clean(detail), n }),
        }
        let oldest = day.saturating_sub(KEEP_DAYS);
        self.days.retain(|d, _| *d >= oldest);
        self.taken.retain(|(_, d)| *d + 1 >= day);
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = String::new();
        for (day, list) in &self.days {
            for s in list {
                out.push_str(&format!("story\t{day}\t{}\t{}\t{}\t{}\n", s.event.index(), s.n, s.who, s.detail));
            }
        }
        let mut t: Vec<_> = self.taken.iter().collect();
        t.sort();
        for (who, day) in t {
            out.push_str(&format!("taken\t{day}\t{who}\n"));
        }
        out.into_bytes()
    }

    pub fn decode(b: &[u8]) -> News {
        let mut out = News::default();
        for line in String::from_utf8_lossy(b).lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f[..] {
                ["story", day, e, n, who, detail] => {
                    let (Ok(day), Ok(e), Ok(n)) = (day.parse::<u32>(), e.parse::<u8>(), n.parse::<u32>()) else { continue };
                    if let Some(event) = Event::from_index(e) {
                        out.days.entry(day).or_default().push(Story { event, who: who.to_string(), detail: detail.to_string(), n });
                    }
                }
                ["taken", day, who] => {
                    if let Ok(day) = day.parse() {
                        out.taken.insert((who.to_string(), day));
                    }
                }
                _ => {}
            }
        }
        out
    }
}

const PLACES: [&str; 8] = ["Hmmington", "Clankbury", "Wheatley", "Bellford", "Muddlecombe", "Cobble Green", "Thatchby", "Gravelton"];
const PREFIXES: [&str; 8] = ["Little ", "Upper ", "Lower ", "Old ", "Great ", "North ", "South ", ""];
const PAPERS: [&str; 4] = ["Gazette", "Herald", "Bugle", "Clarion"];

/// A village's masthead number (from where its square is).
pub fn masthead_of(village: macroquad::math::IVec3) -> u16 {
    let h = (village.x as u32).wrapping_mul(0x9E37_79B1) ^ (village.z as u32).wrapping_mul(0x85EB_CA77);
    ((h >> 7) & 0xFF) as u16
}

/// A village's place name, and its paper's name.
pub fn place_name(m: u16) -> String {
    format!("{}{}", PREFIXES[(m as usize >> 3) % 8], PLACES[m as usize % 8])
}

pub fn paper_name(m: u16) -> String {
    format!("The {} {}", place_name(m), PAPERS[(m as usize >> 6) % 4])
}

/// A paper's wear: which day's issue, and whose masthead.
pub fn paper_wear(day: u32, masthead: u16) -> u32 {
    (day & 0xFFFF) | (masthead as u32) << 16
}

pub fn paper_of(wear: u32) -> (u32, u16) {
    (wear & 0xFFFF, (wear >> 16) as u16)
}

/// Pick one of a few ways of saying it (the same every time for the same issue).
fn pick<'a>(seed: u32, k: usize, options: &[&'a str]) -> &'a str {
    let h = seed.wrapping_mul(0x2545_F491).wrapping_add(k as u32 * 0x9E37) >> 5;
    options[h as usize % options.len()]
}

fn plural(n: u32, one: &str, many: &str) -> String {
    if n == 1 { one.to_string() } else { format!("{n} {many}") }
}

/// An issue as it's read: the paper's name, the day, and the stories (headline, body).
pub type Issue = (String, u32, Vec<(String, String)>);

/// One story as it's printed: a headline and a line under it.
fn write_up(s: &Story, seed: u32, k: usize) -> (String, String) {
    let who = &s.who;
    let n = s.n;
    match s.event {
        Event::TreePunched => (
            format!("Local Player {who} Punches Tree"),
            format!("{} Witnesses describe the tree as \"fine, considering\".", if n > 1 { format!("{n} logs were taken, by hand.") } else { "One log was taken, by hand.".to_string() }),
        ),
        Event::Died => (
            pick(seed, k, &["Tragedy Strikes", "Unfortunate Incident", "Setback for Local Adventurer"]).to_string(),
            format!("{who} {}. They are expected back shortly. Their belongings are said to be \"somewhere around there\".", s.detail.trim_end_matches(['.', '!'])),
        ),
        Event::RaidWon => (pick(seed, k, &["Raid Repelled!", "Pilferers Sent Packing", "Village Stands Firm"]).to_string(), format!("{who} saw off the raiders. Hmmers report feeling \"quite brave, actually\".")),
        Event::RaidLost => ("Raid Ruins Market Day".to_string(), format!("Raiders got the better of {who}. The village says it will remember this.")),
        Event::Hisser => (
            format!("Hisser Seen Near {}", pick(seed, k, &["Bakery", "Well", "Bell", "Library", "Farm"])),
            format!("{} near {who}, and then, briefly, not seen at all. Repairs are under way.", plural(n, "A Hisser was seen", "Hissers were seen")),
        ),
        Event::Master => ("Hmmer Reaches Master".to_string(), format!("A Hmmer is now a Master of its trade{}. Prices are said to reflect this.", if who.is_empty() { String::new() } else { format!(", thanks to {who}") })),
        Event::Clanker => ("Village Builds Clanker".to_string(), format!("{who} has a new Clanker. The Clanker had no comment.")),
        Event::House => (format!("New {} for {who}", if n > 1 { "Houses" } else { "House" }), "Neighbours say they are \"fine with it\". Somebody has already moved in.".to_string()),
        Event::Bounty => (format!("{who} Lends a Hand"), format!("{} on the Notice Board. The village is grateful, and slightly less short of things.", plural(n, "A bounty was done", "bounties were done"))),
        Event::Sale => ("Market Report".to_string(), format!("{n} gold changed hands at {who}'s stall. Economists call it \"a start\".")),
        Event::Baby => (pick(seed, k, &["Baby Boom Continues", "Happy News", "New Arrival"]).to_string(), format!("{} born yesterday. Beds are reported to be in short supply.", plural(n, "A baby Hmmer was", "baby Hmmers were"))),
        Event::Boss => (format!("{who} Defeats {}", s.detail), "Sky no longer falling, say experts. Celebrations are expected to be loud.".to_string()),
        Event::Storm => ("Storm Rattles Rooftops".to_string(), "Thunder and lightning overnight. Anyone holding a sword aloft is asked to stop.".to_string()),
    }
}

/// When nothing much happened.
const QUIET: [(&str, &str); 6] = [
    ("Nothing Happens; Experts Baffled", "A day of almost complete calm. Our reporters remain on alert."),
    ("Fluffer Escapes, Is Found, Was Never Lost", "The Fluffer was in the field the whole time. An inquiry is planned."),
    ("Well Bucket Missing Again", "Letters to the editor: who keeps taking it? Answers on a postcard."),
    ("Weather Continues", "Some sun, some cloud, and the sky in its usual place. More of the same is forecast."),
    ("Local Hmmer Hmms", "Asked to elaborate, the Hmmer hmmed again. We will keep readers informed."),
    ("Prices Steady at the Stalls", "Bread is bread, and gold is gold. Analysts expect this to continue."),
];

/// Small ads, one an issue.
const ADS: [&str; 6] = [
    "FOR SALE: one bucket, slightly holey. Will trade for a better bucket.",
    "LOST: one Clanker's patience. If found, please do not return it.",
    "WANTED: anyone who has seen a diamond. Just to talk.",
    "NOTICE: the Bell is for raids, not for fun. You know who you are.",
    "FREE TO A GOOD HOME: mud. Lots of it. Collect from the swamp.",
    "SERVICES: walls built, roofs mended. No Hissers, please.",
];

/// How important a story is (the lead goes first).
fn weight(s: &Story) -> u32 {
    let base = match s.event {
        Event::Boss => 100,
        Event::RaidWon | Event::RaidLost => 90,
        Event::Died => 70,
        Event::Hisser => 65,
        Event::Master => 60,
        Event::Clanker => 55,
        Event::House => 50,
        Event::TreePunched => 45,
        Event::Bounty => 40,
        Event::Sale => 35,
        Event::Baby => 30,
        Event::Storm => 20,
    };
    base * 100 + s.n.min(99)
}

/// The issue for day `day`: yesterday's news, the most important first,
/// topped up with the usual if it was quiet, and a small ad.
pub fn issue(news: &News, day: u32) -> Vec<(String, String)> {
    let seed = day.wrapping_mul(0x632B_E5AB) ^ 0x51ED;
    if day == 0 {
        return vec![
            ("First Issue!".to_string(), "Welcome to your village paper. Yesterday's news, every morning, from the Notice Board.".to_string()),
            ("World Begins".to_string(), "Witnesses describe it as \"sudden\". More tomorrow.".to_string()),
            ("Small Ads".to_string(), ADS[seed as usize % ADS.len()].to_string()),
        ];
    }
    let mut stories: Vec<Story> = news.days.get(&(day - 1)).cloned().unwrap_or_default();
    stories.sort_by_key(|s| std::cmp::Reverse(weight(s)));
    let mut out: Vec<(String, String)> = stories.iter().take(STORIES - 1).enumerate().map(|(k, s)| write_up(s, seed, k)).collect();
    let mut q = seed as usize;
    while out.len() < 3 {
        let (h, b) = QUIET[q % QUIET.len()];
        if !out.iter().any(|(oh, _)| oh == h) {
            out.push((h.to_string(), b.to_string()));
        }
        q += 1;
    }
    out.push(("Small Ads".to_string(), ADS[(seed as usize >> 3) % ADS.len()].to_string()));
    out
}

impl Game {
    /// Where the world lives: something newsworthy happened.
    pub fn report(&mut self, event: Event, who: &str, detail: &str, n: u32) {
        if self.is_client() {
            return;
        }
        self.news.note(self.day, event, who, detail, n);
    }

    /// The name of the village round a place, for the paper.
    pub fn place_of(&self, at: macroquad::math::Vec3) -> String {
        match self.village_at(at) {
            Some(c) => place_name(masthead_of(c.floor().as_ivec3())),
            None => "the village".to_string(),
        }
    }

    /// Take today's paper from the Notice Board we have open.
    pub fn take_paper(&mut self) {
        let Some(pos) = self.notice else { return };
        if self.is_client() {
            self.net_send_msg(Msg::TakePaper { x: pos.x, y: pos.y, z: pos.z });
            return;
        }
        let me = crate::players::record_key(&self.player_name);
        if let Some(wear) = self.paper_for(pos, &me) {
            self.give_worn(NEWSPAPER, 1, wear);
        } else {
            self.msg("You've had today's paper. (There's only one a day.)");
        }
    }

    /// Today's paper from the board at `pos` for `who`, if they haven't had it.
    fn paper_for(&mut self, pos: macroquad::math::IVec3, who: &str) -> Option<u32> {
        let village = self.village_key(pos.as_vec3());
        if !self.news.taken.insert((who.to_string(), self.day)) {
            return None;
        }
        Some(paper_wear(self.day, masthead_of(village)))
    }

    /// A joined player takes today's paper from a board.
    pub fn host_take_paper(&mut self, from: u32, pos: macroquad::math::IVec3) {
        if !self.peer_near(from, pos) || self.world.get_v(pos) != NOTICE_BOARD || self.village_at(pos.as_vec3()).is_none() {
            return;
        }
        let who = crate::players::record_key(&self.peer_name(from));
        match self.paper_for(pos, &who) {
            Some(wear) => self.give_peer_worn(from, NEWSPAPER, 1, wear),
            None => self.system_message(Some(from), "You've had today's paper. (There's only one a day.)"),
        }
    }

    /// The stories in day `day`'s issue (a joined player asks the host for them, once in a while, until they come).
    pub fn paper_issue(&mut self, day: u32) -> Option<Vec<(String, String)>> {
        if !self.is_client() {
            return Some(issue(&self.news, day));
        }
        if let Some(i) = self.news.issues.get(&day) {
            return Some(i.clone());
        }
        if self.news.ask <= 0.0 {
            self.news.ask = 2.0;
            self.net_send_msg(Msg::PaperAsk { day });
        }
        None
    }

    /// A joined player wants an issue.
    pub fn host_paper_ask(&mut self, from: u32, day: u32) {
        if day > self.day {
            return;
        }
        let stories = issue(&self.news, day);
        self.net_send_to(from, Msg::Paper { day, stories });
    }

    /// Reading a paper: the advancements (once it's been printed for us).
    pub fn read_paper(&mut self, wear: u32, dt: f32) -> Option<Issue> {
        self.news.ask -= dt;
        let (day, masthead) = paper_of(wear);
        let stories = self.paper_issue(day)?;
        self.advance("extra_extra");
        let me = &self.player_name;
        if stories.first().is_some_and(|(h, b)| !me.is_empty() && (h.contains(me.as_str()) || b.starts_with(me.as_str()))) {
            self.advance("front_page");
        }
        Some((paper_name(masthead), day, stories))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_news_adds_up_is_saved_and_forgets_old_days() {
        let mut n = News::default();
        n.note(3, Event::TreePunched, "Ann", "", 1);
        n.note(3, Event::TreePunched, "Ann", "", 1);
        n.note(3, Event::Died, "Bob", "fell out of the world", 1);
        n.note(3, Event::Died, "Bob", "was blown up by a Hisser", 1);
        assert_eq!(n.days[&3].len(), 3, "the same thing adds up; different ends don't");
        assert_eq!(n.days[&3][0].n, 2);
        n.taken.insert(("ann".into(), 3));
        let back = News::decode(&n.encode());
        assert_eq!(back.days, n.days);
        assert_eq!(back.taken, n.taken);
        n.note(3 + KEEP_DAYS + 1, Event::Storm, "", "", 1);
        assert!(!n.days.contains_key(&3), "old news goes");
    }

    #[test]
    fn an_issue_leads_with_the_biggest_story_and_is_never_empty() {
        let mut n = News::default();
        n.note(4, Event::Baby, "Hmmington", "", 2);
        n.note(4, Event::TreePunched, "Ann", "", 1);
        n.note(4, Event::RaidWon, "Little Wheatley", "", 1);
        let i = issue(&n, 5);
        assert!(i[0].0.contains("Raid") || i[0].0.contains("Pilferers") || i[0].0.contains("Stands"), "{i:?}");
        assert!(i.iter().any(|(h, _)| h == "Local Player Ann Punches Tree"));
        assert_eq!(i.last().unwrap().0, "Small Ads");
        assert_eq!(issue(&n, 5), i, "the same issue every time");
        // A quiet day still fills the paper.
        let q = issue(&n, 9);
        assert!(q.len() >= 4, "{q:?}");
        assert_eq!(issue(&n, 0)[0].0, "First Issue!");
    }

    #[test]
    fn mastheads_and_papers() {
        let m = masthead_of(macroquad::math::IVec3::new(120, 70, -40));
        assert!(paper_name(m).starts_with("The "));
        assert_eq!(paper_of(paper_wear(42, m)), (42, m));
        let names: HashSet<String> = (0..40).map(|k| paper_name(masthead_of(macroquad::math::IVec3::new(k * 700, 64, -k * 300)))).collect();
        assert!(names.len() > 10, "villages have their own papers: {names:?}");
    }

    #[test]
    fn a_paper_a_day_from_the_board_and_reading_it() {
        let (mut g, key) = crate::village_growth::tests::village_with_square(11);
        let board = crate::structures::village_board(key - macroquad::math::IVec3::Y);
        g.world.set_v(board, NOTICE_BOARD);
        g.player.body.pos = board.as_vec3() + macroquad::math::Vec3::new(2.5, 0.0, 0.5);
        g.day = 6;
        g.report(Event::TreePunched, &g.player_name.clone(), "", 1);
        g.day = 7;
        g.open_board(board);
        g.take_paper();
        assert_eq!(g.inv.count(NEWSPAPER), 1);
        g.take_paper();
        assert_eq!(g.inv.count(NEWSPAPER), 1, "one a day");
        let slot = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == NEWSPAPER)).unwrap();
        let (title, day, stories) = g.read_paper(g.inv.wear[slot], 0.1).expect("printed");
        assert_eq!(day, 7);
        assert_eq!(title, paper_name(masthead_of(key)));
        assert!(stories[0].0.contains("Punches Tree"), "{stories:?}");
        assert!(g.advancements.earned.iter().any(|a| a == "extra_extra"));
        assert!(g.advancements.earned.iter().any(|a| a == "front_page"), "the lead story is about us");
    }
}
