//! **Languages.** Every bit of text the game shows is written in English in
//! the code, and listed in the strings file, `lang/strings.txt` (kept
//! complete by a test). A translation is a mod: a file that pairs each
//! English line with its translation,
//!
//! ```text
//! Singleplayer = Einzelspieler
//! Render Distance: {} chunks = Sichtweite: {} Chunks
//! {} joined the game = {} ist beigetreten
//! ```
//!
//! and the text is translated where it's drawn (see ui.rs), so everything is
//! covered, menus to messages. `{}` stands for a part filled in as the game
//! runs (a number, a name), which is translated too when it can be; `{1}`,
//! `{2}` put the parts in another order. Lines starting with `#` are notes.
//!
//! Text that players write (chat, signs, books, names) is never translated.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};

/// One line with parts filled in as the game runs.
#[derive(Debug, Clone)]
struct Template {
    /// The English text around the parts: one more than there are parts.
    pieces: Vec<String>,
    /// The translation, split the same way, with which part goes after each piece.
    out: Vec<String>,
    order: Vec<usize>,
}

impl Template {
    fn parse(english: &str, translated: &str) -> Option<Template> {
        let pieces: Vec<String> = english.split("{}").map(str::to_string).collect();
        let n = pieces.len() - 1;
        // A line with no plain text at all would match anything.
        if n == 0 || pieces.iter().all(|p| p.trim().is_empty()) {
            return None;
        }
        // The translation's parts: `{}` in turn, or `{1}`, `{2}`, ... in any order.
        let (mut out, mut order) = (vec![String::new()], Vec::new());
        let mut rest = translated;
        let mut next = 0;
        while let Some(i) = rest.find('{') {
            let close = rest[i..].find('}').map(|c| i + c);
            let inner = close.map(|c| &rest[i + 1..c]);
            let which = match inner {
                Some("") => {
                    next += 1;
                    Some(next - 1)
                }
                Some(k) => k.parse::<usize>().ok().filter(|&k| (1..=n).contains(&k)).map(|k| k - 1),
                None => None,
            };
            let last = out.last_mut()?;
            match (which, close) {
                (Some(w), Some(c)) => {
                    last.push_str(&rest[..i]);
                    order.push(w);
                    out.push(String::new());
                    rest = &rest[c + 1..];
                }
                _ => {
                    last.push_str(&rest[..=i]);
                    rest = &rest[i + 1..];
                }
            }
        }
        out.last_mut()?.push_str(rest);
        (order.len() == n).then_some(Template { pieces, out, order })
    }

    /// The parts, if `text` is this line with them filled in.
    fn capture<'a>(&self, text: &'a str) -> Option<Vec<&'a str>> {
        let first = &self.pieces[0];
        let last = &self.pieces[self.pieces.len() - 1];
        if !text.starts_with(first.as_str()) || !text.ends_with(last.as_str()) || text.len() < first.len() + last.len() {
            return None;
        }
        let body = &text[first.len()..text.len() - last.len()];
        let mut parts = Vec::new();
        let mut at = 0;
        for piece in &self.pieces[1..self.pieces.len() - 1] {
            let found = body[at..].find(piece.as_str())?;
            parts.push(&body[at..at + found]);
            at += found + piece.len();
        }
        parts.push(&body[at..]);
        Some(parts)
    }

    fn fill(&self, parts: &[String]) -> String {
        let mut s = self.out[0].clone();
        for (k, &w) in self.order.iter().enumerate() {
            s.push_str(&parts[w]);
            s.push_str(&self.out[k + 1]);
        }
        s
    }
}

/// Lines translated so far, and what came out.
type Cache = (HashMap<String, Arc<str>>, HashSet<Arc<str>>);

/// A language's lines, ready to look up.
#[derive(Default)]
pub struct Table {
    exact: HashMap<String, String>,
    templates: Vec<Template>,
    /// What's been translated already (and what came out, so it isn't translated twice).
    cache: Mutex<Cache>,
}

/// Most cached lines before the cache starts again (text that changes every
/// frame, like a countdown, would otherwise pile up).
const CACHE_MAX: usize = 8192;

impl Table {
    /// Read a translation file (see the top of this file). Returns the table
    /// and any lines it couldn't make sense of.
    pub fn parse(text: &str) -> (Table, Vec<String>) {
        let mut t = Table::default();
        let mut errs = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            // The first " = " not escaped (`\=` is a plain = in the English).
            let Some((en, tr)) = split_pair(line) else {
                errs.push(format!("line {}: needs English = translation", n + 1));
                continue;
            };
            let en = unescape(&en);
            let tr = unescape(tr.trim());
            if en.contains("{}") {
                match Template::parse(&en, &tr) {
                    Some(tp) => t.templates.push(tp),
                    None => errs.push(format!("line {}: the translation needs one {{}} for each in the English", n + 1)),
                }
            } else {
                t.exact.insert(en, tr);
            }
        }
        // Longest English first, so the most specific line wins.
        t.templates.sort_by_key(|tp| std::cmp::Reverse(tp.pieces.iter().map(String::len).sum::<usize>()));
        (t, errs)
    }

    pub fn len(&self) -> usize {
        self.exact.len() + self.templates.len()
    }

    fn lookup(&self, text: &str, depth: u8) -> Option<String> {
        if let Some(s) = self.exact.get(text) {
            return Some(s.clone());
        }
        // Text with spaces around it keeps them.
        let trimmed = text.trim();
        if trimmed.len() != text.len() && !trimmed.is_empty() && let Some(s) = self.exact.get(trimmed) {
            let start = text.find(trimmed).unwrap_or(0);
            return Some(format!("{}{s}{}", &text[..start], &text[start + trimmed.len()..]));
        }
        if depth > 3 {
            return None;
        }
        for tp in &self.templates {
            if let Some(parts) = tp.capture(text) {
                let parts: Vec<String> = parts.iter().map(|p| self.lookup(p, depth + 1).unwrap_or_else(|| p.to_string())).collect();
                return Some(tp.fill(&parts));
            }
        }
        // Several lines: each on its own.
        if text.contains('\n') {
            let lines: Vec<String> = text.split('\n').map(|l| self.lookup(l, depth + 1).unwrap_or_else(|| l.to_string())).collect();
            return Some(lines.join("\n"));
        }
        None
    }

    /// `text` in this language (or as it is, if there's no translation).
    pub fn translate(&self, text: &str) -> Arc<str> {
        let Ok(mut cache) = self.cache.lock() else { return text.into() };
        if let Some(s) = cache.0.get(text) {
            return s.clone();
        }
        if let Some(s) = cache.1.get(text) {
            return s.clone();
        }
        let out: Arc<str> = self.lookup(text, 0).unwrap_or_else(|| text.to_string()).into();
        if cache.0.len() >= CACHE_MAX {
            cache.0.clear();
            cache.1.clear();
        }
        cache.0.insert(text.to_string(), out.clone());
        cache.1.insert(out.clone());
        out
    }
}

fn split_pair(line: &str) -> Option<(String, &str)> {
    let b = line.as_bytes();
    let mut i = 0;
    while i + 3 <= b.len() {
        if &b[i..i + 3] == b" = " && (i == 0 || b[i - 1] != b'\\') {
            return Some((line[..i].to_string(), &line[i + 3..]));
        }
        i += 1;
    }
    None
}

/// `\n` is a new line, `\=` an equals sign, `\\` a backslash.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some(o) => out.push(o),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A language a mod brings.
#[derive(Clone)]
pub struct Language {
    /// `modid:code`, as kept in the settings.
    pub key: String,
    /// Its own name for itself ("Deutsch").
    pub name: &'static str,
    pub table: Arc<Table>,
    /// A font with the letters the language needs (a .ttf or .otf in the mod), if the built-in one won't do.
    pub font: Option<Arc<Vec<u8>>>,
}

static ACTIVE: RwLock<Option<(String, Arc<Table>)>> = RwLock::new(None);

/// Show the game in this language (None: English).
pub fn set(lang: Option<&Language>) {
    if let Ok(mut a) = ACTIVE.write() {
        *a = lang.map(|l| (l.key.clone(), l.table.clone()));
    }
}

/// The language being shown (its key; empty for English).
pub fn current() -> String {
    ACTIVE.read().ok().and_then(|a| a.as_ref().map(|(k, _)| k.clone())).unwrap_or_default()
}

/// `text` in the language being shown.
pub fn tr(text: &str) -> Cow<'_, str> {
    let table = ACTIVE.read().ok().and_then(|a| a.as_ref().map(|(_, t)| t.clone()));
    match table {
        Some(t) => Cow::Owned(t.translate(text).to_string()),
        None => Cow::Borrowed(text),
    }
}

/// The text of the strings file: every line the game can show, in English.
#[cfg(test)]
pub(crate) fn extract_strings(src: &std::path::Path) -> Vec<String> {
    let mut found = std::collections::BTreeSet::new();
    let mut files = Vec::new();
    let mut dirs = vec![src.to_path_buf()];
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                files.push(p);
            }
        }
    }
    files.sort();
    for f in files {
        let code = std::fs::read_to_string(&f).unwrap_or_default();
        // Tests' text isn't shown to anyone.
        let code = code.split("#[cfg(test)]\nmod tests").next().unwrap_or("");
        for lit in literals(code) {
            if let Some(s) = shown_text(&lit) {
                found.insert(s);
            }
        }
    }
    found.into_iter().collect()
}

/// The string literals in some Rust code (plain "..." ones), still escaped.
#[cfg(test)]
fn literals(code: &str) -> Vec<String> {
    let b = code.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            // Line comments (doc comments included) aren't text.
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            // A char literal: '"' isn't the start of a string.
            b'\'' if b.get(i + 2) == Some(&b'\'') || (b.get(i + 1) == Some(&b'\\') && b.get(i + 3) == Some(&b'\'')) => {
                i += if b[i + 1] == b'\\' { 4 } else { 3 };
                continue;
            }
            // A raw string with #s ends at "#, whatever's inside.
            b'"' if i > 0 && b[i - 1] == b'#' => {
                i += 1;
                while i + 1 < b.len() && !(b[i] == b'"' && b[i + 1] == b'#') {
                    i += 1;
                }
                i += 1;
            }
            b'"' => {
                // Raw and byte strings are never shown text.
                let prefixed = i > 0 && (b[i - 1] == b'r' || b[i - 1] == b'b' || b[i - 1] == b'#');
                let start = i + 1;
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                if !prefixed && i <= b.len() {
                    out.push(code[start..i.min(b.len())].to_string());
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// A literal as a line of the strings file, if it looks like something shown:
/// words with a capital or a space, not a name in the code ("iron_pickaxe"),
/// a path or a format of numbers only. `{x}`, `{:.1}` and the like become `{}`.
#[cfg(test)]
fn shown_text(lit: &str) -> Option<String> {
    let lit = rust_unescape(lit);
    // Format arguments, whatever they hold, are parts filled in.
    let mut s = String::new();
    let mut chars = lit.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                s.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                s.push('}');
            }
            '{' => {
                for d in chars.by_ref() {
                    if d == '}' {
                        break;
                    }
                }
                s.push_str("{}");
            }
            _ => s.push(c),
        }
    }
    let words: String = s.replace("{}", "");
    let letters = words.chars().filter(|c| c.is_alphabetic()).count();
    let has_upper = words.chars().any(|c| c.is_uppercase());
    let has_space = words.trim().contains(' ');
    let code_like = !has_space && (words.contains('_') || words.contains("::") || words.contains('/') || words.contains('.') && !words.ends_with('.'));
    if letters < 2 || code_like || !(has_upper || has_space) || s.starts_with('#') && !has_space {
        return None;
    }
    // Unlikely to be shown: all capitals and short (keys, codes), or a magic number.
    if !has_space && words.len() <= 4 && words.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) && !matches!(words.as_str(), "ON" | "OFF" | "OK") {
        return None;
    }
    Some(s)
}

/// A literal's text: Rust's escapes undone (and a line ended with `\\` joined to the next).
#[cfg(test)]
fn rust_unescape(lit: &str) -> String {
    let mut out = String::new();
    let mut chars = lit.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push(' '),
            Some('\n') => {
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
            }
            Some('u') => {
                let code: String = chars.by_ref().skip(1).take_while(|&c| c != '}').collect();
                out.extend(u32::from_str_radix(&code, 16).ok().and_then(char::from_u32));
            }
            Some(o) => out.push(o),
            None => {}
        }
    }
    out
}

/// A line as written in the strings file (and in translations).
#[cfg(test)]
fn escape_line(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace(" = ", " \\= ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> Table {
        let (t, errs) = Table::parse(text);
        assert!(errs.is_empty(), "{errs:?}");
        t
    }

    #[test]
    fn lines_and_lines_with_parts_are_translated() {
        let t = table(
            "# German, a bit\n\
             Singleplayer = Einzelspieler\n\
             Iron Pickaxe = Eisenspitzhacke\n\
             Render Distance: {} chunks = Sichtweite: {} Chunks\n\
             {} gave {} to {} = {3} bekam {2} von {1}\n\
             Got {} = Bekommen: {}\n\
             a \\= b = a ist b\n",
        );
        assert_eq!(&*t.translate("Singleplayer"), "Einzelspieler");
        assert_eq!(&*t.translate("Render Distance: 12 chunks"), "Sichtweite: 12 Chunks");
        assert_eq!(&*t.translate("Bob gave Iron Pickaxe to Ann"), "Ann bekam Eisenspitzhacke von Bob");
        assert_eq!(&*t.translate("Got Iron Pickaxe"), "Bekommen: Eisenspitzhacke");
        assert_eq!(&*t.translate("  Singleplayer "), "  Einzelspieler ");
        assert_eq!(&*t.translate("a = b"), "a ist b");
        assert_eq!(&*t.translate("Iron Pickaxe\nSingleplayer"), "Eisenspitzhacke\nEinzelspieler");
        // What it doesn't know stays English, and a translation isn't translated again.
        assert_eq!(&*t.translate("Quit to Desktop"), "Quit to Desktop");
        assert_eq!(&*t.translate("Einzelspieler"), "Einzelspieler");
        assert_eq!(t.len(), 6);
    }

    #[test]
    fn bad_lines_are_reported() {
        let (_, errs) = Table::parse("no separator here\nHello {} = Hallo\n{} = {}\n");
        assert_eq!(errs.len(), 3, "{errs:?}");
    }

    #[test]
    fn literals_and_shown_text_are_found() {
        let code = r#"let a = "Hello there"; // "Not this"
            let b = format!("Render Distance: {d} chunks"); let c = 'x'; let q = '"';
            let k = "iron_pickaxe"; let p = "saves/world.mncr"; let e = "{x:.1}"; let f = "Options";"#;
        let shown: Vec<String> = literals(code).iter().filter_map(|l| shown_text(l)).collect();
        assert_eq!(shown, vec!["Hello there", "Render Distance: {} chunks", "Options"]);
        // Escapes are undone, and written back the way translations read them.
        let line = shown_text(r#"Two\nlines say \"a = b\""#).unwrap();
        assert_eq!(line, "Two\nlines say \"a = b\"");
        let (t, _) = Table::parse(&format!("{} = ok", escape_line(&line)));
        assert_eq!(&*t.translate(&line), "ok");
    }

    /// The strings file lists every line the game can show. Run with
    /// MINCERAFT_WRITE_STRINGS=1 to bring it up to date after changing text.
    #[test]
    fn the_strings_file_is_complete() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut text = String::from(STRINGS_HEADER);
        for s in extract_strings(&root.join("src")) {
            text.push_str(&escape_line(&s));
            text.push('\n');
        }
        let path = root.join("lang").join("strings.txt");
        if std::env::var("MINCERAFT_WRITE_STRINGS").is_ok() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &text).unwrap();
        }
        let have = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
        assert!(have == text, "lang/strings.txt is out of date: run the tests with MINCERAFT_WRITE_STRINGS=1");
    }

    const STRINGS_HEADER: &str = "# Every line of text Minceraft shows, in English: the starting point for a\n\
# translation (see MODDING.md, Languages). Copy the lines you translate into\n\
# your mod's language file as `English = translation`. {} is a part filled in\n\
# as the game runs. (Made from the code by a test; some lines are never seen.)\n";
}
