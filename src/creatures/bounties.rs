//! **Notice boards and bounties** (v0.5): each village puts up a Notice
//! Board on its square (see village_growth.rs), and pins three bounties to
//! it: two things it's short of ("bring 20 wheat") and a nuisance it would
//! like seen off ("defeat 5 Rattlers").
//!
//! - **Bring** bounties are handed in at the board, all in one go.
//! - **Defeat** bounties count what you've beaten (anywhere: they're not
//!   fussy) since the bounties went up. A Desert Groaner counts as a
//!   Groaner, and a Snowy or Charred Rattler as a Rattler.
//! - Each pays gold and experience once per player, and the village thinks
//!   a little better of you for it (see reputation.rs).
//! - New bounties go up every other day, the same for everyone.
//!
//! A board only works in a village (anyone can build one, but nobody pins
//! anything to it out in the wilds). Joined players ask the host what their
//! board says, and the host checks what they hand in.

use crate::block::*;
use crate::entity::MobKind;
use crate::game::Game;
use crate::net::Msg;
use macroquad::math::{IVec3, Vec3};
use std::collections::{HashMap, HashSet};

/// Bounties on a board.
pub const PER_BOARD: usize = 3;
/// Days each lot of bounties stays up.
pub const DAYS: u32 = 2;
/// How near (blocks) a board has to be to use it.
pub const REACH: f32 = 6.0;
/// How much better a village thinks of you for each one.
pub const REP: i32 = 3;
/// Experience points for each gold ingot a bounty pays.
const XP_PER_GOLD: u16 = 5;
/// Bounties finished for "Odd Jobs".
pub const ODD_JOBS: u32 = 10;

/// (thing, how many, gold paid).
const BRING: [(Id, u8, u8); 16] = [
    (WHEAT, 20, 3),
    (CARROT, 16, 3),
    (POTATO, 16, 3),
    (BREAD, 6, 3),
    (PUMPKIN, 6, 3),
    (APPLE, 8, 2),
    (MELON_SLICE, 24, 3),
    (COAL, 16, 4),
    (LOG, 16, 3),
    (COBBLE, 32, 2),
    (WOOL, 8, 4),
    (STRING, 8, 3),
    (FEATHER, 10, 2),
    (BONE, 10, 3),
    (GUNPOWDER, 5, 5),
    (IRON, 4, 6),
];
/// (nuisance, how many, gold paid).
const DEFEAT: [(MobKind, u8, u8); 5] = [(MobKind::Groaner, 5, 4), (MobKind::Rattler, 5, 4), (MobKind::Webber, 4, 4), (MobKind::Hisser, 3, 6), (MobKind::Witch, 1, 6)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    Bring(Id, u8),
    Defeat(MobKind, u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounty {
    pub task: Task,
    pub gold: u8,
    pub xp: u16,
}

impl Bounty {
    /// How many it wants.
    pub fn wants(&self) -> u16 {
        match self.task {
            Task::Bring(_, n) | Task::Defeat(_, n) => n as u16,
        }
    }

    /// What it says on the note.
    pub fn text(&self) -> String {
        match self.task {
            Task::Bring(id, n) => format!("Bring {n} {}", short_name(id)),
            Task::Defeat(k, 1) => format!("Defeat a {}", k.name()),
            Task::Defeat(k, n) => format!("Defeat {n} {}s", k.name()),
        }
    }
}

/// An item's name without the joke in brackets.
fn short_name(id: Id) -> &'static str {
    let name = item_name(id);
    name.split(" (").next().unwrap_or(name)
}

/// A bounty as one player sees it: how far along they are, and whether they've done it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub bounty: Bounty,
    pub have: u16,
    pub done: bool,
}

/// Which lot of bounties is up on day `day`.
pub fn posting(day: u32) -> u32 {
    day / DAYS
}

/// A village's bounties for a posting: the same for everyone, and new every other day.
pub fn bounties(village: IVec3, posting: u32) -> [Bounty; PER_BOARD] {
    let seed = (village.x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (village.z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ (posting as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    let mut rng = crate::noise::Rng::new(seed | 1);
    let a = rng.int(0, BRING.len() as i32 - 1) as usize;
    let b = (a + 1 + rng.int(0, BRING.len() as i32 - 2) as usize) % BRING.len();
    let d = rng.int(0, DEFEAT.len() as i32 - 1) as usize;
    let bring = |i: usize| {
        let (id, n, gold) = BRING[i];
        Bounty { task: Task::Bring(id, n), gold, xp: gold as u16 * XP_PER_GOLD }
    };
    let (kind, n, gold) = DEFEAT[d];
    [bring(a), bring(b), Bounty { task: Task::Defeat(kind, n), gold, xp: gold as u16 * XP_PER_GOLD }]
}

/// Does beating a `killed` count towards a bounty for `want`s?
pub fn counts_for(want: MobKind, killed: MobKind) -> bool {
    want == killed || matches!((want, killed), (MobKind::Groaner, MobKind::DesertGroaner) | (MobKind::Rattler, MobKind::SnowyRattler | MobKind::CharredRattler))
}

/// Who has done what (kept where the world lives, and saved with it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bounties {
    /// The posting the rest is about.
    pub posting: u32,
    /// (player record key, mob kind index) -> how many beaten this posting.
    pub kills: HashMap<(String, u8), u16>,
    /// (village, bounty, player record key): done this posting.
    pub done: HashSet<(IVec3, u8, String)>,
    /// Bounties each player has ever finished.
    pub total: HashMap<String, u32>,
    /// (Not saved.) What the host last said our board says (a joined player's game).
    pub seen: Option<(IVec3, Vec<Row>)>,
}

impl Bounties {
    /// Move on to day `day`'s bounties (forgetting the old ones' progress).
    pub fn refresh(&mut self, day: u32) {
        if posting(day) != self.posting {
            self.posting = posting(day);
            self.kills.clear();
            self.done.clear();
        }
    }

    /// How many `want`s `who` has beaten this posting.
    pub fn beaten(&self, who: &str, want: MobKind) -> u16 {
        self.kills.iter().filter(|((w, k), _)| w == who && MobKind::from_index(*k).is_some_and(|k| counts_for(want, k))).map(|(_, n)| *n).sum()
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = format!("posting\t{}\n", self.posting);
        let mut kills: Vec<_> = self.kills.iter().collect();
        kills.sort();
        for ((who, k), n) in kills {
            out.push_str(&format!("kill\t{who}\t{k}\t{n}\n"));
        }
        let mut done: Vec<_> = self.done.iter().collect();
        done.sort_by(|a, b| (a.0.to_array(), a.1, &a.2).cmp(&(b.0.to_array(), b.1, &b.2)));
        for (v, slot, who) in done {
            out.push_str(&format!("done\t{}\t{}\t{}\t{slot}\t{who}\n", v.x, v.y, v.z));
        }
        let mut total: Vec<_> = self.total.iter().collect();
        total.sort();
        for (who, n) in total {
            out.push_str(&format!("total\t{who}\t{n}\n"));
        }
        out.into_bytes()
    }

    pub fn decode(b: &[u8]) -> Bounties {
        let mut out = Bounties::default();
        for line in String::from_utf8_lossy(b).lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f[..] {
                ["posting", p] => out.posting = p.parse().unwrap_or(0),
                ["kill", who, k, n] => {
                    if let (Ok(k), Ok(n)) = (k.parse(), n.parse()) {
                        out.kills.insert((who.to_string(), k), n);
                    }
                }
                ["done", x, y, z, slot, who] => {
                    if let (Ok(x), Ok(y), Ok(z), Ok(slot)) = (x.parse(), y.parse(), z.parse(), slot.parse()) {
                        out.done.insert((IVec3::new(x, y, z), slot, who.to_string()));
                    }
                }
                ["total", who, n] => {
                    if let Ok(n) = n.parse() {
                        out.total.insert(who.to_string(), n);
                    }
                }
                _ => {}
            }
        }
        out
    }
}

impl Game {
    /// Right-clicked a Notice Board.
    pub fn open_board(&mut self, pos: IVec3) {
        if self.village_at(pos.as_vec3()).is_none() {
            self.msg("The board is blank. (Nobody out here needs a hand.)");
            return;
        }
        self.notice = Some(pos);
        self.bounties.seen = None;
        if self.is_client() {
            self.net_send_msg(Msg::BoardAsk { x: pos.x, y: pos.y, z: pos.z });
        }
        self.sfx(crate::sound::Sfx::Click, Some(pos.as_vec3()));
    }

    /// The board we have open: its village's name for us, and its bounties as we
    /// see them (and whether it's still there).
    pub fn board_rows(&self) -> Option<(i32, Vec<Row>)> {
        let pos = self.notice?;
        if self.world.get_v(pos) != NOTICE_BOARD || (pos.as_vec3() + Vec3::splat(0.5)).distance(self.player.body.pos) > REACH + 2.0 {
            return None;
        }
        let me = crate::players::record_key(&self.player_name);
        let mut rows = if self.is_client() {
            // (Until the host says, the board is empty.)
            self.bounties.seen.as_ref().filter(|(at, _)| *at == pos).map(|(_, r)| r.clone()).unwrap_or_default()
        } else {
            self.rows_for(pos, &me)
        };
        // What we'd hand in is in our own pockets.
        for r in rows.iter_mut() {
            if let Task::Bring(id, _) = r.bounty.task {
                r.have = (self.inv.count(id) as u16).min(r.bounty.wants());
            }
        }
        let rep = if self.is_client() { self.board_rep } else { self.rep.get(self.village_key(pos.as_vec3()), &me) };
        Some((rep, rows))
    }

    /// A board's bounties as `who` sees them (where the world lives; Bring counts are filled in by whoever looks).
    pub fn rows_for(&self, pos: IVec3, who: &str) -> Vec<Row> {
        let village = self.village_key(pos.as_vec3());
        let posting = posting(self.day);
        let list = bounties(village, posting);
        let same = self.bounties.posting == posting;
        list.iter()
            .enumerate()
            .map(|(i, b)| Row {
                bounty: *b,
                have: match b.task {
                    Task::Defeat(k, n) if same => self.bounties.beaten(who, k).min(n as u16),
                    _ => 0,
                },
                done: same && self.bounties.done.contains(&(village, i as u8, who.to_string())),
            })
            .collect()
    }

    /// The local player finishes bounty `slot` on the board they have open.
    pub fn finish_bounty(&mut self, slot: usize) {
        let Some(pos) = self.notice else { return };
        if !self.is_client() {
            self.bounties.refresh(self.day);
        }
        let Some((_, rows)) = self.board_rows() else { return };
        let Some(r) = rows.get(slot).copied() else { return };
        if r.done {
            return;
        }
        if r.have < r.bounty.wants() {
            self.msg(match r.bounty.task {
                Task::Bring(..) => "Not enough yet. (They did say how many.)",
                Task::Defeat(..) => "Not done yet. (They're still about.)",
            });
            return;
        }
        if self.is_client() {
            // The host checks and pays (and tells us what the board says now).
            if let Task::Bring(id, n) = r.bounty.task
                && !self.creative
            {
                self.inv.remove(id, n as u32);
            }
            if let Some((_, seen)) = self.bounties.seen.as_mut()
                && let Some(s) = seen.get_mut(slot)
            {
                s.done = true;
            }
            self.net_send_msg(Msg::Bounty { x: pos.x, y: pos.y, z: pos.z, slot: slot as u8 });
            self.sfx(crate::sound::Sfx::Hmm, None);
            return;
        }
        if let Task::Bring(id, n) = r.bounty.task
            && !self.creative
        {
            self.inv.remove(id, n as u32);
        }
        self.give(GOLD_INGOT, r.bounty.gold);
        self.add_xp(r.bounty.xp as u32);
        let me = crate::players::record_key(&self.player_name);
        self.bounty_paid(pos, slot as u8, &me);
    }

    /// Where the world lives: `who` has finished bounty `slot` at the board at `pos`.
    fn bounty_paid(&mut self, pos: IVec3, slot: u8, who: &str) {
        let village = self.village_key(pos.as_vec3());
        self.bounties.done.insert((village, slot, who.to_string()));
        let total = {
            let t = self.bounties.total.entry(who.to_string()).or_insert(0);
            *t += 1;
            *t
        };
        self.change_rep(village, who, REP);
        self.tell(who, "The village thanks you. (There's a note pinned up saying so.)");
        self.sfx(crate::sound::Sfx::Hmm, Some(pos.as_vec3()));
        self.advance_for(who, "help_wanted");
        if total >= ODD_JOBS {
            self.advance_for(who, "odd_jobs");
        }
    }

    /// A joined player wants to see a board.
    pub fn host_board_ask(&mut self, from: u32, pos: IVec3) {
        if !self.peer_near(from, pos) || self.world.get_v(pos) != NOTICE_BOARD || self.village_at(pos.as_vec3()).is_none() {
            return;
        }
        self.send_board(from, pos);
    }

    fn send_board(&mut self, to: u32, pos: IVec3) {
        let who = crate::players::record_key(&self.peer_name(to));
        let rows = self.rows_for(pos, &who);
        let rep = self.rep.get(self.village_key(pos.as_vec3()), &who).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        let rows = rows
            .iter()
            .map(|r| match r.bounty.task {
                Task::Bring(id, n) => (0, id, n, r.bounty.gold, r.bounty.xp, r.have, r.done),
                Task::Defeat(k, n) => (1, k.index() as Id, n, r.bounty.gold, r.bounty.xp, r.have, r.done),
            })
            .collect();
        self.net_send_to(to, Msg::Board { x: pos.x, y: pos.y, z: pos.z, rep, rows });
    }

    /// A joined player's board, from the host.
    pub fn board_from_host(&mut self, pos: IVec3, rep: i16, rows: Vec<(u8, Id, u8, u8, u16, u16, bool)>) {
        let rows = rows
            .into_iter()
            .filter_map(|(kind, what, n, gold, xp, have, done)| {
                let task = match kind {
                    0 => Task::Bring(what, n),
                    _ => Task::Defeat(MobKind::from_index(what as u8)?, n),
                };
                Some(Row { bounty: Bounty { task, gold, xp }, have, done })
            })
            .collect();
        self.bounties.seen = Some((pos, rows));
        self.board_rep = rep as i32;
    }

    /// A joined player finishes a bounty: check they're at the board and have done it.
    pub fn host_bounty(&mut self, from: u32, pos: IVec3, slot: u8) {
        if !self.peer_near(from, pos) || self.world.get_v(pos) != NOTICE_BOARD || self.village_at(pos.as_vec3()).is_none() {
            return;
        }
        let who = crate::players::record_key(&self.peer_name(from));
        self.bounties.refresh(self.day);
        let Some(r) = self.rows_for(pos, &who).get(slot as usize).copied() else { return };
        let ready = match r.bounty.task {
            Task::Bring(id, n) => !r.done && (self.peer_free(from) || self.peers.get(&from).is_some_and(|p| p.ledger.bag.count(id) >= n as u32)),
            Task::Defeat(..) => !r.done && r.have >= r.bounty.wants(),
        };
        if ready {
            if let Task::Bring(id, n) = r.bounty.task {
                self.peer_take(from, id, n as u32);
            }
            self.give_peer(from, GOLD_INGOT, r.bounty.gold);
            self.give_peer_xp(from, r.bounty.xp as u32);
            self.bounty_paid(pos, slot, &who);
        }
        self.send_board(from, pos);
    }

    /// Where the world lives: `who` beat a `kind`.
    pub fn bounty_kill(&mut self, who: &str, kind: MobKind) {
        if !kind.hostile() {
            return;
        }
        self.bounties.refresh(self.day);
        *self.bounties.kills.entry((crate::players::record_key(who), kind.index())).or_insert(0) += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boards_post_three_bounties_that_change_every_other_day() {
        let v = IVec3::new(120, 70, -40);
        let a = bounties(v, 5);
        assert_eq!(a, bounties(v, 5), "the same for everyone");
        assert!(matches!(a[0].task, Task::Bring(..)) && matches!(a[1].task, Task::Bring(..)) && matches!(a[2].task, Task::Defeat(..)));
        assert_ne!(a[0].task, a[1].task, "two different things");
        assert!(a.iter().all(|b| b.gold > 0 && b.xp == b.gold as u16 * XP_PER_GOLD));
        assert_eq!(posting(2 * DAYS), posting(2 * DAYS + 1));
        assert_ne!(posting(2 * DAYS), posting(3 * DAYS));
        // Over a while, a board asks for all sorts.
        let kinds: HashSet<String> = (0..60).map(|p| bounties(v, p)[2].text()).collect();
        assert!(kinds.len() >= 4, "{kinds:?}");
        assert_eq!(Bounty { task: Task::Defeat(MobKind::Rattler, 5), gold: 4, xp: 20 }.text(), "Defeat 5 Rattlers");
        assert_eq!(Bounty { task: Task::Bring(WHEAT, 20), gold: 3, xp: 15 }.text(), "Bring 20 Wheat");
    }

    #[test]
    fn cousins_count_and_the_record_is_saved() {
        assert!(counts_for(MobKind::Rattler, MobKind::SnowyRattler));
        assert!(counts_for(MobKind::Groaner, MobKind::DesertGroaner));
        assert!(!counts_for(MobKind::Groaner, MobKind::Rattler));
        let mut b = Bounties::default();
        b.refresh(9);
        b.kills.insert(("ann".into(), MobKind::SnowyRattler.index()), 2);
        b.kills.insert(("ann".into(), MobKind::Rattler.index()), 1);
        b.done.insert((IVec3::new(1, 2, 3), 1, "ann".into()));
        b.total.insert("ann".into(), 4);
        assert_eq!(b.beaten("ann", MobKind::Rattler), 3);
        assert_eq!(b.beaten("bob", MobKind::Rattler), 0);
        let back = Bounties::decode(&b.encode());
        assert_eq!(back, b);
        // A new posting starts afresh (but the totals stay).
        b.refresh(9 + DAYS);
        assert!(b.kills.is_empty() && b.done.is_empty() && b.total["ann"] == 4);
    }

    #[test]
    fn bring_and_defeat_bounties_pay_once_each() {
        let (mut g, key) = crate::village_growth::tests::village_with_square(11);
        let board = crate::structures::village_board(key - IVec3::Y);
        g.world.set_v(board, NOTICE_BOARD);
        g.player.body.pos = board.as_vec3() + Vec3::new(2.5, 0.0, 0.5);
        g.open_board(board);
        let (_, rows) = g.board_rows().expect("the board is open");
        assert_eq!(rows.len(), PER_BOARD);
        // Not enough yet.
        let Task::Bring(item, n) = rows[0].bounty.task else { panic!() };
        g.finish_bounty(0);
        assert!(!g.board_rows().unwrap().1[0].done);
        g.give(item, n);
        let gold = g.inv.count(GOLD_INGOT);
        g.finish_bounty(0);
        assert!(g.board_rows().unwrap().1[0].done);
        assert_eq!(g.inv.count(item), 0, "handed in");
        assert_eq!(g.inv.count(GOLD_INGOT), gold + rows[0].bounty.gold as u32);
        assert!(g.advancements.earned.iter().any(|a| a == "help_wanted"));
        assert!(g.board_rows().unwrap().0 > 0, "the village thinks better of us");
        // Only once.
        g.give(item, n);
        g.finish_bounty(0);
        assert_eq!(g.inv.count(item), n as u32);
        // Beating things counts (cousins too).
        let Task::Defeat(kind, n) = rows[2].bounty.task else { panic!() };
        let me = g.player_name.clone();
        for _ in 0..n {
            g.bounty_kill(&me, kind);
        }
        assert_eq!(g.board_rows().unwrap().1[2].have, n as u16);
        g.finish_bounty(2);
        assert!(g.board_rows().unwrap().1[2].done);
        // Out in the wilds, a board is just a board.
        let wild = (1..).map(|k| IVec3::new(k * 300, 60, -k * 300)).find(|p| g.village_at(p.as_vec3()).is_none()).unwrap();
        g.world.set_v(wild, NOTICE_BOARD);
        g.notice = None;
        g.open_board(wild);
        assert!(g.notice.is_none());
    }
}
