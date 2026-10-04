//! Books you write yourself, and lecterns to read them on.
//!
//! - A **Book and Quill** (a book, a feather and some coal for ink): use it
//!   to write, up to twenty pages. Sign it (give it a title) and it becomes a
//!   **Written Book** with your name on it, which anyone can read but nobody
//!   can change.
//! - A **Lectern** (four planks round a bookshelf): use a book on it to put
//!   the book there, then anyone can read it; take it back from the reading
//!   screen. Breaking the lectern drops the book.
//!
//! A book's words are kept where the world lives, under the number in the
//! book's wear (like a Hollow Box's contents). Joined players' hosts store
//! what they write (checking they own the book) and send the words of any
//! book they want to read.

use crate::block::*;
use crate::boxes::{box_id, box_wear};
use crate::game::Game;
use crate::net::Msg;
use crate::sound::{Mat, Sfx};
use macroquad::math::{IVec3, Vec3};
use std::collections::HashMap;

pub const MAX_PAGES: usize = 20;
pub const PAGE_LEN: usize = 256;
pub const TITLE_LEN: usize = 32;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Book {
    pub title: String,
    pub author: String,
    pub pages: Vec<String>,
}

impl Book {
    /// Trimmed to what a book can hold (other players can't be trusted).
    pub fn clean(mut self) -> Book {
        let cut = |s: &str, n: usize| s.chars().filter(|c| !c.is_control() || *c == '\n').take(n).collect::<String>();
        self.title = cut(&self.title, TITLE_LEN);
        self.author = cut(&self.author, 32);
        self.pages.truncate(MAX_PAGES);
        for p in self.pages.iter_mut() {
            *p = cut(p, PAGE_LEN);
        }
        if self.pages.is_empty() {
            self.pages.push(String::new());
        }
        self
    }
}

/// What's open on the book screen.
#[derive(Clone, Debug)]
pub struct BookView {
    pub book: Book,
    /// Our own Book and Quill (in this inventory slot): we can write in it.
    pub writing: Option<usize>,
    /// Read off this lectern (and can be taken from it).
    pub lectern: Option<IVec3>,
}

pub fn is_book(id: Id) -> bool {
    id == BOOK_AND_QUILL || id == WRITTEN_BOOK
}

pub fn is_lectern(id: Id) -> bool {
    id == LECTERN || id == LECTERN_BOOK
}

/// Books and lecterns for the save.
pub fn encode(books: &HashMap<u16, Book>, lecterns: &HashMap<IVec3, (Id, u16)>) -> Vec<u8> {
    let mut out = Vec::new();
    let s = |out: &mut Vec<u8>, t: &str| {
        let b = t.as_bytes();
        out.extend((b.len().min(u16::MAX as usize) as u16).to_le_bytes());
        out.extend(&b[..b.len().min(u16::MAX as usize)]);
    };
    out.extend((books.len() as u32).to_le_bytes());
    for (id, b) in books {
        out.extend(id.to_le_bytes());
        s(&mut out, &b.title);
        s(&mut out, &b.author);
        out.push(b.pages.len().min(MAX_PAGES) as u8);
        for p in b.pages.iter().take(MAX_PAGES) {
            s(&mut out, p);
        }
    }
    out.extend((lecterns.len() as u32).to_le_bytes());
    for (p, (item, tag)) in lecterns {
        for c in [p.x, p.y, p.z] {
            out.extend(c.to_le_bytes());
        }
        out.extend(item.to_le_bytes());
        out.extend(tag.to_le_bytes());
    }
    out
}

pub fn decode(b: &[u8]) -> (HashMap<u16, Book>, HashMap<IVec3, (Id, u16)>) {
    struct R<'a>(&'a [u8], usize);
    impl R<'_> {
        fn take(&mut self, n: usize) -> Option<&[u8]> {
            let s = self.0.get(self.1..self.1 + n)?;
            self.1 += n;
            Some(s)
        }
        fn u16(&mut self) -> Option<u16> {
            Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
        }
        fn u32(&mut self) -> Option<u32> {
            Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
        }
        fn text(&mut self) -> Option<String> {
            let len = self.u16()? as usize;
            Some(String::from_utf8_lossy(self.take(len)?).into_owned())
        }
    }
    let mut r = R(b, 0);
    let mut books = HashMap::new();
    let mut lecterns = HashMap::new();
    let mut read = || -> Option<()> {
        for _ in 0..r.u32()?.min(100_000) {
            let id = r.u16()?;
            let (title, author) = (r.text()?, r.text()?);
            let pages_n = r.take(1)?[0] as usize;
            let mut pages = Vec::new();
            for _ in 0..pages_n {
                pages.push(r.text()?);
            }
            books.insert(id, Book { title, author, pages }.clean());
        }
        for _ in 0..r.u32()?.min(100_000) {
            let (x, y, z) = (r.u32()? as i32, r.u32()? as i32, r.u32()? as i32);
            let (item, tag) = (r.u16()?, r.u16()?);
            lecterns.insert(IVec3::new(x, y, z), (item, tag));
        }
        Some(())
    };
    let _ = read();
    (books, lecterns)
}

impl Game {
    /// The words in the book tagged `tag` (a joined player's copy is the host's word).
    #[cfg(test)]
    pub fn book(&self, tag: u16) -> Option<&Book> {
        self.books.get(&tag)
    }

    /// Open our held book: to write in (a Book and Quill) or to read.
    pub fn open_held_book(&mut self) {
        let i = self.inv.selected;
        let Some((item, _)) = self.inv.slots[i] else { return };
        let tag = box_id(self.inv.wear[i]);
        self.player.swing = 1.0;
        if self.is_client() && tag != 0 && !self.books.contains_key(&tag) {
            // Ask for the words first; the screen opens when they arrive.
            self.book_waiting = Some((tag, if item == BOOK_AND_QUILL { Some(i) } else { None }, None));
            self.net_send_msg(Msg::BookAsk { tag, x: 0, y: i32::MIN, z: 0 });
            return;
        }
        let book = self.books.get(&tag).cloned().unwrap_or_default().clean();
        self.reading = Some(BookView { book, writing: (item == BOOK_AND_QUILL).then_some(i), lectern: None });
    }

    /// Done writing in the Book and Quill in `slot`: keep the words, and (`title`) sign it.
    pub fn finish_writing(&mut self, slot: usize, pages: Vec<String>, title: Option<String>) {
        if self.inv.slots[slot].map(|s| s.0) != Some(BOOK_AND_QUILL) {
            return;
        }
        let tag = box_id(self.inv.wear[slot]);
        let author = self.player_name.clone();
        if self.is_client() {
            self.book_pending = Some(slot);
            self.net_send_msg(Msg::BookWrite { tag, title: title.clone().unwrap_or_default(), pages: pages.clone(), sign: title.is_some() });
            // Show it straight away; the host's word follows.
            let book = Book { title: title.clone().unwrap_or_default(), author, pages }.clean();
            if tag != 0 {
                self.books.insert(tag, book);
            }
            return;
        }
        let (new, signed) = self.store_book(tag, title.clone(), author, pages);
        self.inv.wear[slot] = box_wear(new);
        if signed {
            self.inv.slots[slot] = Some((WRITTEN_BOOK, 1));
            self.sfx(Sfx::Place(Mat::Wood), None);
            self.advance("published");
        }
    }

    /// Where the world lives: keep a book's words. Returns (its tag, signed).
    pub fn store_book(&mut self, tag: u16, title: Option<String>, author: String, pages: Vec<String>) -> (u16, bool) {
        let id = match tag {
            0 => (1..=u16::MAX).find(|k| !self.books.contains_key(k) && *k != 0).unwrap_or(1),
            t => t,
        };
        let signed = title.as_deref().is_some_and(|t| !t.trim().is_empty());
        let book = Book { title: title.unwrap_or_default(), author: if signed { author } else { String::new() }, pages }.clean();
        self.books.insert(id, book);
        (id, signed)
    }

    /// A joined player wrote in (or signed) one of their books.
    pub fn host_book_write(&mut self, from: u32, tag: u16, title: String, pages: Vec<String>, sign: bool) {
        let owns = self.peer_free(from) || self.peers.get(&from).is_some_and(|p| if tag == 0 { p.ledger.bag.has(BOOK_AND_QUILL) } else { p.ledger.owns_enchanted(BOOK_AND_QUILL, tag) });
        if !owns {
            return;
        }
        let author = self.peer_name(from);
        let (new, signed) = self.store_book(tag, sign.then_some(title), author, pages);
        if let Some(l) = self.peers.get_mut(&from).map(|p| &mut p.ledger) {
            if tag != 0 {
                l.remove_enchanted(BOOK_AND_QUILL, tag);
            }
            if signed {
                // The Book and Quill is a Written Book now.
                l.bag.take(BOOK_AND_QUILL, 1);
                l.bag.add(WRITTEN_BOOK, 1);
                l.add_enchanted(WRITTEN_BOOK, new, 1);
            } else {
                l.add_enchanted(BOOK_AND_QUILL, new, 1);
            }
        }
        let name = self.peer_name(from);
        if signed {
            self.advance_for(&crate::players::record_key(&name), "published");
        }
        self.send_book(from, tag, new, signed, false);
    }

    /// Send a joined player a book's words (and, `open`, open it for reading).
    pub fn send_book(&mut self, to: u32, old: u16, new: u16, signed: bool, open: bool) {
        let b = self.books.get(&new).cloned().unwrap_or_default();
        self.net_send_to(to, Msg::BookState { old, new, signed, open, title: b.title, author: b.author, pages: b.pages });
    }

    /// The host's word on a book: keep the words, retag ours, and open it if we were waiting.
    #[allow(clippy::too_many_arguments)]
    pub fn book_state(&mut self, old: u16, new: u16, signed: bool, open: bool, title: String, author: String, pages: Vec<String>) {
        let book = Book { title, author, pages }.clean();
        if new != 0 {
            self.books.insert(new, book.clone());
        }
        if let Some(i) = self.book_pending.take()
            && self.inv.slots[i].is_some_and(|s| is_book(s.0))
            && box_id(self.inv.wear[i]) == old
        {
            self.inv.wear[i] = box_wear(new);
            if signed {
                self.inv.slots[i] = Some((WRITTEN_BOOK, 1));
            }
        }
        if let Some((tag, writing, lectern)) = self.book_waiting
            && (tag == new || open)
        {
            self.book_waiting = None;
            self.reading = Some(BookView { book, writing, lectern });
        }
    }

    /// Right-clicked a lectern holding `held`: put a book on it, or read the one there.
    pub fn use_lectern(&mut self, p: IVec3, held: Id) {
        self.player.swing = 1.0;
        let id = self.world.get_v(p);
        if id == LECTERN && is_book(held) {
            let i = self.inv.selected;
            let tag = box_id(self.inv.wear[i]);
            if self.is_client() {
                self.inv.consume_held();
                self.net_send_msg(Msg::Interact { x: p.x, y: p.y, z: p.z, item: held });
                return;
            }
            self.lecterns.insert(p, (held, tag));
            self.world.set_v(p, LECTERN_BOOK);
            if !self.creative {
                self.inv.consume_held();
            }
            self.sfx(Sfx::Place(Mat::Wood), Some(p.as_vec3() + Vec3::splat(0.5)));
            return;
        }
        if id == LECTERN_BOOK {
            if self.is_client() {
                self.book_waiting = Some((u16::MAX, None, Some(p)));
                self.net_send_msg(Msg::BookAsk { tag: 0, x: p.x, y: p.y, z: p.z });
                return;
            }
            let tag = self.lecterns.get(&p).map(|l| l.1).unwrap_or(0);
            let book = self.books.get(&tag).cloned().unwrap_or_default().clean();
            self.reading = Some(BookView { book, writing: None, lectern: Some(p) });
        }
    }

    /// A joined player put a book on a lectern (they must own it).
    pub fn host_lectern_put(&mut self, from: u32, p: IVec3, item: Id) {
        if self.world.get_v(p) != LECTERN || !is_book(item) {
            return;
        }
        let tag = if self.verified_held(from) == item { self.verified_ench(from) } else { 0 };
        if !self.peer_take(from, item, 1) {
            return;
        }
        if tag != 0
            && let Some(l) = self.peers.get_mut(&from).map(|p| &mut p.ledger)
        {
            l.remove_enchanted(item, tag);
        }
        self.lecterns.insert(p, (item, tag));
        self.world.set_v(p, LECTERN_BOOK);
    }

    /// A joined player wants a book's words: by its tag, or off a lectern.
    pub fn host_book_ask(&mut self, from: u32, tag: u16, p: IVec3) {
        if p.y == i32::MIN {
            self.send_book(from, tag, tag, false, false);
        } else if let Some(&(_, tag)) = self.lecterns.get(&p) {
            self.send_book(from, tag, tag, false, true);
        }
    }

    /// Take the book off a lectern (into our inventory).
    pub fn take_from_lectern(&mut self, p: IVec3) {
        if self.is_client() {
            self.net_send_msg(Msg::LecternTake { x: p.x, y: p.y, z: p.z });
            return;
        }
        let me = self.my_id;
        self.lectern_give(p, me);
    }

    /// Where the world lives: the lectern's book goes to `who`.
    pub fn lectern_give(&mut self, p: IVec3, who: u32) {
        let Some((item, tag)) = self.lecterns.remove(&p) else { return };
        self.world.set_v(p, LECTERN);
        if who == self.my_id && !self.dedicated {
            self.give_worn(item, 1, box_wear(tag));
        } else {
            self.give_peer_worn(who, item, 1, box_wear(tag));
        }
    }

    /// A joined player took the book off a lectern.
    pub fn host_lectern_take(&mut self, from: u32, p: IVec3) {
        let near = self.peers.get(&from).is_some_and(|q| q.target.distance(p.as_vec3()) < 8.0);
        if near {
            self.lectern_give(p, from);
        }
    }

    /// A lectern broke: its book falls out.
    pub fn spill_lectern(&mut self, p: IVec3) {
        if let Some((item, tag)) = self.lecterns.remove(&p) {
            self.pop_drop_worn(p.as_vec3() + Vec3::splat(0.5), item, 1, box_wear(tag));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tests::arena;

    #[test]
    fn write_sign_and_read_on_a_lectern() {
        let mut g = arena(71);
        g.inv.slots[0] = Some((BOOK_AND_QUILL, 1));
        g.inv.selected = 0;
        g.open_held_book();
        assert!(g.reading.as_ref().is_some_and(|r| r.writing == Some(0)));
        g.finish_writing(0, vec!["Once upon a time".into(), "The end.".into()], None);
        let tag = box_id(g.inv.wear[0]);
        assert!(tag != 0);
        assert_eq!(g.book(tag).unwrap().pages.len(), 2);
        g.finish_writing(0, vec!["Once upon a time".into(), "The end, really.".into()], Some("My Story".into()));
        assert_eq!(g.inv.slots[0], Some((WRITTEN_BOOK, 1)));
        let b = g.book(box_id(g.inv.wear[0])).unwrap().clone();
        assert_eq!((b.title.as_str(), b.pages[1].as_str()), ("My Story", "The end, really."));
        assert!(!b.author.is_empty());
        // Onto a lectern, read it, take it back.
        let p = g.player.body.pos.floor().as_ivec3() + IVec3::new(2, 0, 0);
        g.world.set_v(p, LECTERN);
        g.use_lectern(p, WRITTEN_BOOK);
        assert_eq!(g.world.get_v(p), LECTERN_BOOK);
        assert_eq!(g.inv.slots[0], None);
        g.reading = None;
        g.use_lectern(p, AIR);
        assert_eq!(g.reading.as_ref().map(|r| r.book.title.clone()), Some("My Story".to_string()));
        g.take_from_lectern(p);
        assert_eq!(g.world.get_v(p), LECTERN);
        assert_eq!(g.inv.count(WRITTEN_BOOK), 1);
        // Saved and loaded.
        let (books, _) = decode(&encode(&g.books, &g.lecterns));
        assert_eq!(books.get(&tag).map(|b| b.title.as_str()), Some("My Story"));
    }
}
