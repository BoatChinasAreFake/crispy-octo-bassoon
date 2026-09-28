//! Data-driven mods. A mod is a folder in `mods/` containing a `mod.txt`
//! (plus optional 16x16 PNG textures). Mods add blocks, items, recipes,
//! textures, world generation and simple behaviours; they never run code,
//! so they're safe to share, and servers send theirs to joining players.
//! See MODDING.md for the file format.

use crate::block::build::{def, item, leak};
use crate::block::*;
use crate::noise::Rng;
use crate::texture::{base_texture, FIRST_MOD_TILE, TILE};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const MAX_FILE: usize = 1 << 20;
pub const MAX_PACK: usize = 4 << 20;

/// One mod's files, from disk or from a server.
#[derive(Clone, Debug, PartialEq)]
pub struct ModSource {
    pub id: String,
    pub files: BTreeMap<String, Vec<u8>>,
}

/// The mods behind the current registry, so a host can send them to players.
static ACTIVE: Mutex<Vec<ModSource>> = Mutex::new(Vec::new());

pub fn mods_dir() -> PathBuf {
    PathBuf::from("mods")
}

fn sanitize_id(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect::<String>().to_ascii_lowercase()
}

/// Read every mod on disk: `mods/<id>/mod.txt (+ .png files)` or `mods/<id>.txt`.
pub fn read_disk(dir: &Path) -> Vec<ModSource> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for e in entries.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            let id = sanitize_id(&name);
            let mut files = BTreeMap::new();
            if let Ok(inner) = std::fs::read_dir(&path) {
                for f in inner.flatten() {
                    let fname = f.file_name().to_string_lossy().to_string();
                    let lower = fname.to_ascii_lowercase();
                    if (lower.ends_with(".txt") || lower.ends_with(".png")) && f.path().is_file() {
                        if let Ok(bytes) = std::fs::read(f.path()) {
                            if bytes.len() <= MAX_FILE {
                                files.insert(fname, bytes);
                            }
                        }
                    }
                }
            }
            if files.contains_key("mod.txt") && !id.is_empty() {
                out.push(ModSource { id, files });
            }
        } else if name.ends_with(".txt") && name != "disabled.txt" {
            let id = sanitize_id(name.trim_end_matches(".txt"));
            if let (false, Ok(bytes)) = (id.is_empty(), std::fs::read(&path)) {
                let mut files = BTreeMap::new();
                files.insert("mod.txt".to_string(), bytes);
                out.push(ModSource { id, files });
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out.dedup_by(|a, b| a.id == b.id);
    out
}

pub fn read_disabled(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("disabled.txt"))
        .map(|s| s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default()
}

pub fn set_enabled(dir: &Path, id: &str, enabled: bool) -> std::io::Result<()> {
    let mut d = read_disabled(dir);
    d.retain(|x| x != id);
    if !enabled {
        d.push(id.to_string());
    }
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("disabled.txt"), d.join("\n") + "\n")
}

/// Load mods from disk and make them active. Returns the new registry's mod list.
pub fn install_local() -> Vec<ModInfo> {
    let dir = mods_dir();
    let sources = read_disk(&dir);
    let disabled = read_disabled(&dir);
    install_sources(sources, &disabled)
}

/// Make these mods active (used for local mods and for a server's mod pack).
pub fn install_sources(sources: Vec<ModSource>, disabled: &[String]) -> Vec<ModInfo> {
    let reg = build(&sources, disabled);
    let infos = reg.mods.clone();
    *ACTIVE.lock().unwrap() = sources.into_iter().filter(|s| !disabled.contains(&s.id)).collect();
    install(reg);
    infos
}

/// The active mods, packed for sending to a joining player.
pub fn active_pack() -> Vec<u8> {
    encode_pack(&ACTIVE.lock().unwrap())
}

pub fn encode_pack(mods: &[ModSource]) -> Vec<u8> {
    let mut b = Vec::new();
    let put = |b: &mut Vec<u8>, bytes: &[u8]| {
        b.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        b.extend_from_slice(bytes);
    };
    b.extend_from_slice(&(mods.len() as u32).to_le_bytes());
    for m in mods {
        put(&mut b, m.id.as_bytes());
        b.extend_from_slice(&(m.files.len() as u32).to_le_bytes());
        for (name, data) in &m.files {
            put(&mut b, name.as_bytes());
            put(&mut b, data);
        }
    }
    b
}

pub fn decode_pack(mut b: &[u8]) -> Result<Vec<ModSource>, String> {
    if b.len() > MAX_PACK {
        return Err("mod pack too large".into());
    }
    fn u32_(b: &mut &[u8]) -> Result<usize, String> {
        if b.len() < 4 {
            return Err("truncated mod pack".into());
        }
        let v = u32::from_le_bytes(b[..4].try_into().unwrap()) as usize;
        *b = &b[4..];
        Ok(v)
    }
    fn bytes<'a>(b: &mut &'a [u8]) -> Result<&'a [u8], String> {
        let n = u32_(b)?;
        if n > b.len() || n > MAX_FILE {
            return Err("bad mod pack entry".into());
        }
        let (a, rest) = b.split_at(n);
        *b = rest;
        Ok(a)
    }
    let count = u32_(&mut b)?;
    if count > 256 {
        return Err("too many mods in pack".into());
    }
    let mut out = Vec::new();
    for _ in 0..count {
        let id = sanitize_id(&String::from_utf8_lossy(bytes(&mut b)?));
        let nfiles = u32_(&mut b)?;
        if nfiles > 256 {
            return Err("too many files in pack".into());
        }
        let mut files = BTreeMap::new();
        for _ in 0..nfiles {
            let name = String::from_utf8_lossy(bytes(&mut b)?).to_string();
            let data = bytes(&mut b)?.to_vec();
            // Only plain names: no paths, no surprises.
            if !name.contains(['/', '\\']) && !name.starts_with('.') {
                files.insert(name, data);
            }
        }
        if !id.is_empty() {
            out.push(ModSource { id, files });
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------ parsing

struct Section {
    kind: String,
    name: String,
    line: usize,
    kv: Vec<(String, String, usize)>,
    rows: Vec<(String, usize)>,
}

impl Section {
    fn get(&self, key: &str) -> Option<&(String, String, usize)> {
        self.kv.iter().find(|(k, _, _)| k == key)
    }
    fn str(&self, key: &str) -> Option<&str> {
        self.get(key).map(|(_, v, _)| v.as_str())
    }
}

fn parse(text: &str) -> (Vec<Section>, Vec<String>) {
    let mut out: Vec<Section> = Vec::new();
    let mut errors = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line_no = n + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let inner = line[1..line.len() - 1].trim();
            let mut parts = inner.splitn(2, char::is_whitespace);
            let kind = parts.next().unwrap_or("").to_ascii_lowercase();
            let name = parts.next().unwrap_or("").trim().to_string();
            out.push(Section { kind, name, line: line_no, kv: Vec::new(), rows: Vec::new() });
            continue;
        }
        let Some(sec) = out.last_mut() else {
            errors.push(format!("line {line_no}: text before the first [section]"));
            continue;
        };
        match line.split_once('=') {
            // Pixel rows may contain '=' characters only if they're 16 wide without spaces.
            Some((k, v)) if !(sec.kind == "texture" && line.len() == 16 && !line.contains(' ')) => {
                sec.kv.push((k.trim().to_ascii_lowercase(), v.trim().to_string(), line_no));
            }
            _ => sec.rows.push((line.to_string(), line_no)),
        }
    }
    (out, errors)
}

struct Ctx<'a> {
    reg: &'a mut Registry,
    /// "modid:name" -> texture tile
    textures: HashMap<String, u16>,
    next_tile: u16,
}

fn num<T: std::str::FromStr>(s: &Section, key: &str, default: T, errs: &mut Vec<String>) -> T {
    match s.get(key) {
        None => default,
        Some((_, v, line)) => v.parse().unwrap_or_else(|_| {
            errs.push(format!("line {line}: \"{key}\" should be a number, got \"{v}\""));
            default
        }),
    }
}

fn flag(s: &Section, key: &str, default: bool, errs: &mut Vec<String>) -> bool {
    match s.get(key) {
        None => default,
        Some((_, v, line)) => match v.to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" | "on" => true,
            "false" | "no" | "0" | "off" => false,
            _ => {
                errs.push(format!("line {line}: \"{key}\" should be true or false"));
                default
            }
        },
    }
}

fn parse_color(v: &str) -> Option<[u8; 4]> {
    let v = v.trim();
    if let Some(hex) = v.strip_prefix('#') {
        let p = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
        return match hex.len() {
            6 => Some([p(0)?, p(2)?, p(4)?, 255]),
            8 => Some([p(0)?, p(2)?, p(4)?, p(6)?]),
            _ => None,
        };
    }
    let parts: Vec<u8> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    match parts.len() {
        3 => Some([parts[0], parts[1], parts[2], 255]),
        4 => Some([parts[0], parts[1], parts[2], parts[3]]),
        _ => None,
    }
}

impl Ctx<'_> {
    fn resolve(&self, modid: &str, name: &str) -> Option<u8> {
        let name = name.trim().to_ascii_lowercase();
        if name.contains(':') {
            return self.reg.lookup(&name);
        }
        self.reg.lookup(&format!("{modid}:{name}")).or_else(|| self.reg.lookup(&name))
    }

    fn texture(&mut self, m: &ModSource, v: &str, errs: &mut Vec<String>, line: usize) -> u16 {
        let v = v.trim();
        if v.to_ascii_lowercase().ends_with(".png") {
            let key = format!("{}:{}", m.id, v);
            if let Some(&t) = self.textures.get(&key) {
                return t;
            }
            match m.files.get(v).map(|b| decode_png(b)) {
                Some(Ok(px)) => {
                    if let Some(t) = self.alloc_tile(px, errs, line) {
                        self.textures.insert(key, t);
                        return t;
                    }
                }
                Some(Err(e)) => errs.push(format!("line {line}: couldn't read {v}: {e}")),
                None => errs.push(format!("line {line}: file {v} not found in the mod folder")),
            }
            return crate::texture::T_WHITE;
        }
        let local = format!("{}:{}", m.id, v.to_ascii_lowercase());
        if let Some(&t) = self.textures.get(&local).or_else(|| self.textures.get(&v.to_ascii_lowercase())) {
            return t;
        }
        if let Some(t) = base_texture(&v.to_ascii_lowercase()) {
            return t;
        }
        errs.push(format!("line {line}: unknown texture \"{v}\""));
        crate::texture::T_WHITE
    }

    fn alloc_tile(&mut self, px: Vec<u8>, errs: &mut Vec<String>, line: usize) -> Option<u16> {
        if self.next_tile > 255 {
            errs.push(format!("line {line}: out of texture space (160 mod textures max)"));
            return None;
        }
        let t = self.next_tile;
        self.next_tile += 1;
        self.reg.textures.push((t, px));
        Some(t)
    }

    fn actions(&self, modid: &str, v: &str, errs: &mut Vec<String>, line: usize) -> Vec<Action> {
        let mut out = Vec::new();
        for part in v.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            let (verb, arg) = part.split_once(char::is_whitespace).map(|(a, b)| (a, b.trim())).unwrap_or((part, ""));
            let f = |d: f32| arg.split_whitespace().next().and_then(|x| x.parse::<f32>().ok()).unwrap_or(d);
            let a = match verb.to_ascii_lowercase().as_str() {
                "heal" => Some(Action::Heal(f(4.0).clamp(0.0, 20.0))),
                "explode" => Some(Action::Explode(f(3.0).clamp(0.5, 6.0))),
                "launch" => Some(Action::Launch(f(12.0).clamp(-40.0, 40.0))),
                "message" | "say" => Some(Action::Message(leak(arg))),
                "give" => {
                    let mut w = arg.split_whitespace();
                    let what = w.next().unwrap_or("");
                    let n = w.next().and_then(|x| x.parse().ok()).unwrap_or(1u8).clamp(1, 64);
                    self.resolve(modid, what).map(|id| Action::Give(id, n)).or_else(|| {
                        errs.push(format!("line {line}: give: unknown item \"{what}\""));
                        None
                    })
                }
                "time" => match arg {
                    "day" => Some(Action::SetTime(0.05)),
                    "noon" => Some(Action::SetTime(0.25)),
                    "night" => Some(Action::SetTime(0.55)),
                    "midnight" => Some(Action::SetTime(0.75)),
                    x => x.parse::<f32>().ok().map(|t| Action::SetTime(t.rem_euclid(1.0))),
                },
                "spawn" => match arg.to_ascii_lowercase().as_str() {
                    "oinker" | "pig" => Some(Action::Spawn(0)),
                    "hisser" => Some(Action::Spawn(1)),
                    "groaner" | "zombie" => Some(Action::Spawn(2)),
                    _ => None,
                },
                _ => None,
            };
            match a {
                Some(a) => out.push(a),
                None => errs.push(format!("line {line}: don't understand action \"{part}\"")),
            }
        }
        out
    }
}

fn decode_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = macroquad::texture::Image::from_file_with_format(bytes, None).map_err(|e| e.to_string())?;
    let (w, h) = (img.width as usize, img.height as usize);
    if w == 0 || h == 0 {
        return Err("empty image".into());
    }
    // Nearest-neighbour resample to 16x16.
    let mut out = vec![0u8; TILE * TILE * 4];
    for y in 0..TILE {
        for x in 0..TILE {
            let (sx, sy) = (x * w / TILE, y * h / TILE);
            let i = (sy * w + sx) * 4;
            out[(y * TILE + x) * 4..(y * TILE + x) * 4 + 4].copy_from_slice(&img.bytes[i..i + 4]);
        }
    }
    Ok(out)
}

/// Build a registry: the base game plus every enabled mod, in id order.
pub fn build(sources: &[ModSource], disabled: &[String]) -> Registry {
    let mut reg = Registry::base();
    let mut parsed: Vec<(usize, Vec<Section>, Vec<String>)> = Vec::new();

    // Pass 1: declare every block, item and texture so mods can refer to each other.
    for (mi, m) in sources.iter().enumerate() {
        let enabled = !disabled.contains(&m.id);
        let mut info = ModInfo { id: m.id.clone(), name: m.id.clone(), enabled, ..Default::default() };
        let text = String::from_utf8_lossy(m.files.get("mod.txt").map(|v| v.as_slice()).unwrap_or(&[])).to_string();
        let (sections, mut errs) = parse(&text);
        if let Some(s) = sections.iter().find(|s| s.kind == "mod") {
            info.name = s.str("name").unwrap_or(&m.id).to_string();
            info.version = s.str("version").unwrap_or("").to_string();
            info.author = s.str("author").unwrap_or("").to_string();
            info.description = s.str("description").unwrap_or("").to_string();
        }
        if !enabled {
            reg.mods.push(info);
            continue;
        }
        for s in &sections {
            let key = format!("{}:{}", m.id, s.name.to_ascii_lowercase());
            match s.kind.as_str() {
                "block" | "item" if s.name.is_empty() || s.name.contains(':') => {
                    errs.push(format!("line {}: [{} ...] needs a simple name like [{} ruby]", s.line, s.kind, s.kind));
                }
                "block" => {
                    if reg.blocks.len() >= FIRST_ITEM as usize {
                        errs.push(format!("line {}: too many blocks across all mods ({} max)", s.line, FIRST_ITEM - NUM_BLOCKS));
                    } else if reg.lookup(&key).is_some() {
                        errs.push(format!("line {}: {} is defined twice", s.line, s.name));
                    } else {
                        let mut b = def(leak(&key), leak(&s.name), Model::Cube, true, true, [crate::texture::T_WHITE; 3], 1.0, 0, false, AIR, 0.0, 0);
                        b.drop = reg.blocks.len() as u8;
                        reg.blocks.push(b);
                        info.added.0 += 1;
                    }
                }
                "item" => {
                    if reg.items.len() + FIRST_ITEM as usize > 255 {
                        errs.push(format!("line {}: too many items across all mods", s.line));
                    } else if reg.lookup(&key).is_some() {
                        errs.push(format!("line {}: {} is defined twice", s.line, s.name));
                    } else {
                        reg.items.push(item(leak(&key), leak(&s.name), crate::texture::T_WHITE));
                        info.added.1 += 1;
                    }
                }
                _ => {}
            }
        }
        info.errors = errs;
        reg.mods.push(info);
        parsed.push((mi, sections, Vec::new()));
    }

    // Pass 2: textures first (blocks and items use them), then everything else.
    let mut ctx = Ctx { reg: &mut reg, textures: HashMap::new(), next_tile: FIRST_MOD_TILE };
    for (mi, sections, errs) in parsed.iter_mut() {
        let m = &sources[*mi];
        for s in sections.iter().filter(|s| s.kind == "texture") {
            if s.name.is_empty() {
                errs.push(format!("line {}: [texture] needs a name", s.line));
                continue;
            }
            if let Some(px) = build_texture(&mut ctx, m, s, errs) {
                if let Some(t) = ctx.alloc_tile(px, errs, s.line) {
                    ctx.textures.insert(format!("{}:{}", m.id, s.name.to_ascii_lowercase()), t);
                }
            }
        }
    }
    for (mi, sections, errs) in parsed.iter_mut() {
        let m = &sources[*mi];
        for s in sections.iter() {
            match s.kind.as_str() {
                "block" => fill_block(&mut ctx, m, s, errs),
                "item" => fill_item(&mut ctx, m, s, errs),
                "recipe" => match parse_recipe(&ctx, m, s, errs) {
                    Some(r) => {
                        ctx.reg.recipes.push(r);
                        if let Some(info) = ctx.reg.mods.iter_mut().find(|i| i.id == m.id) {
                            info.added.2 += 1;
                        }
                    }
                    None => {}
                },
                "ore" => {
                    let b = s.str("block").and_then(|v| ctx.resolve(&m.id, v)).filter(|&b| b < FIRST_ITEM);
                    let replace = s.str("replace").map(|v| ctx.resolve(&m.id, v)).unwrap_or(Some(STONE)).filter(|&b| b < FIRST_ITEM);
                    match (b, replace) {
                        (Some(block), Some(replace)) => ctx.reg.ores.push(OreGen {
                            block,
                            replace,
                            min_y: num(s, "min_y", 1, errs),
                            max_y: num(s, "max_y", 60, errs),
                            chance: num(s, "chance", 0.004f32, errs).clamp(0.0, 0.05),
                        }),
                        _ => errs.push(format!("line {}: [ore] needs block = <a block> (and an optional replace = <block>)", s.line)),
                    }
                }
                "plant" => {
                    let b = s.str("block").and_then(|v| ctx.resolve(&m.id, v)).filter(|&b| b < FIRST_ITEM);
                    let on = s.str("on").map(|v| ctx.resolve(&m.id, v)).unwrap_or(Some(GRASS)).filter(|&b| b < FIRST_ITEM);
                    match (b, on) {
                        (Some(block), Some(on)) => ctx.reg.plants.push(PlantGen { block, on, chance: num(s, "chance", 0.01f32, errs).clamp(0.0, 0.2) }),
                        _ => errs.push(format!("line {}: [plant] needs block = <a block> (and an optional on = <block>)", s.line)),
                    }
                }
                "splashes" => ctx.reg.splashes.extend(s.rows.iter().map(|(r, _)| r.chars().take(60).collect::<String>())),
                "mod" | "texture" => {}
                other => errs.push(format!("line {}: unknown section [{other}]", s.line)),
            }
        }
    }
    for (mi, _, errs) in parsed {
        let id = &sources[mi].id;
        if let Some(info) = reg.mods.iter_mut().find(|i| &i.id == id) {
            info.errors.extend(errs);
        }
    }
    reg
}

fn build_texture(ctx: &mut Ctx, m: &ModSource, s: &Section, errs: &mut Vec<String>) -> Option<Vec<u8>> {
    let mut px = vec![0u8; TILE * TILE * 4];
    if let Some((_, file, line)) = s.get("file") {
        return match m.files.get(file.as_str()).map(|b| decode_png(b)) {
            Some(Ok(p)) => Some(p),
            Some(Err(e)) => {
                errs.push(format!("line {line}: couldn't read {file}: {e}"));
                None
            }
            None => {
                errs.push(format!("line {line}: file {file} not found in the mod folder"));
                None
            }
        };
    }
    let mut has_base = false;
    if let Some((_, v, line)) = s.get("base") {
        let tile = ctx.texture(m, v, errs, *line);
        // Copy from already-built mod tiles, or remember to copy from the base atlas later.
        if let Some((_, p)) = ctx.reg.textures.iter().find(|(t, _)| *t == tile) {
            px.copy_from_slice(p);
        } else {
            px = base_tile_pixels(tile);
        }
        has_base = true;
    }
    if let Some((_, v, line)) = s.get("noise") {
        match parse_color(v) {
            Some(c) => {
                let var: f32 = num(s, "variation", 0.12, errs);
                let mut rng = Rng::new(s.name.bytes().fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64)));
                for p in px.chunks_mut(4) {
                    let f = 1.0 + rng.range(-var, var);
                    for i in 0..3 {
                        p[i] = (c[i] as f32 * f).clamp(0.0, 255.0) as u8;
                    }
                    p[3] = c[3];
                }
                has_base = true;
            }
            None => errs.push(format!("line {line}: noise needs a colour like 200,180,60 or #c8b43c")),
        }
    }
    let mut palette: HashMap<char, [u8; 4]> = HashMap::new();
    for (k, v, line) in &s.kv {
        if let Some(ch) = k.strip_prefix("color").map(str::trim) {
            match (ch.chars().next(), parse_color(v)) {
                (Some(c), Some(col)) if ch.chars().count() == 1 => {
                    palette.insert(c, col);
                }
                _ => errs.push(format!("line {line}: write colours like  color # = 200,180,60")),
            }
        }
    }
    if !s.rows.is_empty() {
        if s.rows.len() != TILE {
            errs.push(format!("line {}: a pixel texture needs exactly 16 rows (found {})", s.line, s.rows.len()));
        }
        for (y, (row, line)) in s.rows.iter().take(TILE).enumerate() {
            if row.chars().count() != TILE {
                errs.push(format!("line {line}: pixel rows must be 16 characters wide"));
            }
            for (x, ch) in row.chars().take(TILE).enumerate() {
                let i = (y * TILE + x) * 4;
                match palette.get(&ch) {
                    Some(c) => px[i..i + 4].copy_from_slice(c),
                    None if ch == '.' => {
                        if !has_base {
                            px[i..i + 4].copy_from_slice(&[0, 0, 0, 0]);
                        }
                    }
                    None => errs.push(format!("line {line}: no colour defined for '{ch}' (add  color {ch} = r,g,b)")),
                }
            }
        }
    } else if !has_base {
        errs.push(format!("line {}: texture needs pixel rows, noise = <colour>, base = <texture> or file = <png>", s.line));
        return None;
    }
    Some(px)
}

/// Pixels of a base-game tile (painted fresh; the base atlas is deterministic).
fn base_tile_pixels(tile: u16) -> Vec<u8> {
    use std::sync::OnceLock;
    static ATLAS: OnceLock<Vec<u8>> = OnceLock::new();
    let atlas = ATLAS.get_or_init(|| crate::texture::build_atlas(1337));
    crate::texture::tile_pixels(atlas, tile)
}

fn fill_block(ctx: &mut Ctx, m: &ModSource, s: &Section, errs: &mut Vec<String>) {
    let Some(id) = ctx.reg.lookup(&format!("{}:{}", m.id, s.name.to_ascii_lowercase())) else { return };
    if id >= FIRST_ITEM {
        return;
    }
    let tex_all = s.get("texture").map(|(_, v, l)| ctx.texture(m, v, errs, *l));
    let mut face = |key: &str| s.get(key).map(|(_, v, l)| ctx.texture(m, v, errs, *l));
    let (top, side, bottom) = (face("top"), face("side"), face("bottom"));
    let side = side.or(tex_all).unwrap_or(crate::texture::T_WHITE);
    let tex = [top.or(tex_all).unwrap_or(side), side, bottom.or(tex_all).unwrap_or(side)];
    let model = match s.str("model").unwrap_or("cube").to_ascii_lowercase().as_str() {
        "cube" => Model::Cube,
        "cross" | "plant" => Model::Cross,
        other => {
            errs.push(format!("line {}: model should be cube or cross, not {other}", s.line));
            Model::Cube
        }
    };
    let transparent = flag(s, "transparent", false, errs);
    let cutout = flag(s, "cutout", false, errs);
    let cube = model == Model::Cube;
    let drop = match s.str("drops").map(str::to_ascii_lowercase).as_deref() {
        None | Some("self") => id,
        Some("none") | Some("nothing") => AIR,
        Some(other) => ctx.resolve(&m.id, other).unwrap_or_else(|| {
            errs.push(format!("line {}: drops: unknown item \"{other}\"", s.line));
            id
        }),
    };
    let hardness = match s.str("hardness") {
        Some("unbreakable") => -1.0,
        _ => num(s, "hardness", 1.0f32, errs).min(60.0),
    };
    let sound = match s.str("sound").unwrap_or(if cube { "stone" } else { "grass" }) {
        "stone" => 0,
        "wood" => 1,
        "grass" => 2,
        "sand" => 3,
        "glass" => 4,
        other => {
            errs.push(format!("line {}: sound should be stone, wood, grass, sand or glass, not {other}", s.line));
            0
        }
    };
    let on_break = s.get("on_break").map(|(_, v, l)| ctx.actions(&m.id, v, errs, *l)).unwrap_or_default();
    let tool_pick = s.str("tool").map(|t| t.eq_ignore_ascii_case("pickaxe")).unwrap_or(false);
    let b = &mut ctx.reg.blocks[id as usize];
    if let Some(n) = s.str("name") {
        b.name = leak(n);
    }
    b.tex = tex;
    b.model = model;
    b.solid = flag(s, "solid", cube, errs);
    b.see_through = cube && transparent;
    b.opaque = cube && !transparent && !cutout;
    b.hardness = hardness;
    b.pick_block = tool_pick;
    b.pick_tier = num(s, "tier", if tool_pick { 1u8 } else { 0 }, errs).min(4);
    b.drop = drop;
    b.light = num(s, "light", 0.0f32, errs).clamp(0.0, 15.0);
    b.sound = sound;
    b.bounce = num(s, "bounce", 0.0f32, errs).clamp(0.0, 1.5);
    b.speed = num(s, "speed", 1.0f32, errs).clamp(0.2, 3.0);
    b.on_break = on_break;
    b.creative = flag(s, "creative", true, errs);
}

fn fill_item(ctx: &mut Ctx, m: &ModSource, s: &Section, errs: &mut Vec<String>) {
    let Some(id) = ctx.reg.lookup(&format!("{}:{}", m.id, s.name.to_ascii_lowercase())) else { return };
    if id < FIRST_ITEM {
        return;
    }
    let tile = s.get("texture").map(|(_, v, l)| ctx.texture(m, v, errs, *l)).unwrap_or(crate::texture::T_WHITE);
    let on_use = s.get("on_use").map(|(_, v, l)| ctx.actions(&m.id, v, errs, *l)).unwrap_or_default();
    let food = s.get("food").map(|_| num(s, "food", 1.0f32, errs).clamp(0.0, 20.0));
    let it = &mut ctx.reg.items[(id - FIRST_ITEM) as usize];
    if let Some(n) = s.str("name") {
        it.name = leak(n);
    }
    it.tile = tile;
    it.stack = num(s, "stack", 64u8, errs).clamp(1, 64);
    it.pick_tier = num(s, "pickaxe", 0u8, errs).min(4);
    it.damage = num(s, "damage", 1.0f32, errs).clamp(0.0, 40.0);
    it.food = food;
    it.on_use = on_use;
    it.consume = flag(s, "consume", true, errs);
}

fn parse_stack(ctx: &Ctx, modid: &str, v: &str, errs: &mut Vec<String>, line: usize) -> Option<(u8, u8)> {
    let mut w = v.split_whitespace();
    let name = w.next()?;
    let n: u8 = w.next().map(|x| x.parse().unwrap_or(0)).unwrap_or(1);
    if !(1..=64).contains(&n) {
        errs.push(format!("line {line}: count for {name} should be 1 to 64"));
        return None;
    }
    match ctx.resolve(modid, name) {
        Some(id) if id != AIR => Some((id, n)),
        _ => {
            errs.push(format!("line {line}: unknown item \"{name}\""));
            None
        }
    }
}

fn parse_recipe(ctx: &Ctx, m: &ModSource, s: &Section, errs: &mut Vec<String>) -> Option<Recipe> {
    let (Some((_, inputs, il)), Some((_, output, ol))) = (s.get("inputs"), s.get("output")) else {
        errs.push(format!("line {}: [recipe] needs inputs = ... and output = ...", s.line));
        return None;
    };
    let ins: Vec<(u8, u8)> = inputs.split(',').filter(|p| !p.trim().is_empty()).filter_map(|p| parse_stack(ctx, &m.id, p, errs, *il)).collect();
    let out = parse_stack(ctx, &m.id, output, errs, *ol)?;
    if ins.is_empty() || ins.len() > 6 {
        errs.push(format!("line {il}: a recipe needs 1 to 6 inputs"));
        return None;
    }
    Some(Recipe { inputs: ins, output: out })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHEESE: &str = r#"
[mod]
name = Cheese Mod
version = 1.0

// A block with a pixel-art texture.
[texture wheel]
color # = 240,200,60
color o = 200,160,40
################
#oo##########oo#
################
################
################
######oo########
################
################
################
################
###oo###########
################
################
##########oo####
################
################

[texture slice]
noise = #f0c83c
variation = 0.2

[block wheel]
name = Cheese Wheel
texture = wheel
hardness = 0.8
sound = wood
bounce = 0.8

[block cheese_ore]
name = Cheese Ore
texture = stone
tool = pickaxe
tier = 1
drops = slice

[item slice]
name = Cheese Slice
texture = slice
food = 3

[item cheese_wand]
name = Wand of Dairy
texture = stick
stack = 1
on_use = launch 15; message Wheee!; give slice 2

[recipe]
inputs = slice 4
output = wheel 1

[recipe]
inputs = wheel 1, stick 1
output = cheese_wand

[ore]
block = cheese_ore
max_y = 40
chance = 0.01

[splashes]
Now with cheese!

[nonsense]
"#;

    fn cheese() -> ModSource {
        let mut files = BTreeMap::new();
        files.insert("mod.txt".to_string(), CHEESE.as_bytes().to_vec());
        ModSource { id: "cheese".into(), files }
    }

    #[test]
    fn a_mod_adds_content() {
        let reg = build(&[cheese()], &[]);
        let info = &reg.mods[0];
        assert_eq!(info.name, "Cheese Mod");
        assert_eq!(info.added, (2, 2, 2));
        // The unknown section is reported, nothing else.
        assert_eq!(info.errors.len(), 1, "{:?}", info.errors);
        assert!(info.errors[0].contains("nonsense"));

        let wheel = reg.lookup("cheese:wheel").unwrap();
        let slice = reg.lookup("cheese:slice").unwrap();
        let ore = reg.lookup("cheese:cheese_ore").unwrap();
        assert_eq!(wheel, NUM_BLOCKS);
        assert_eq!(slice, FIRST_MOD_ITEM);
        let w = &reg.blocks[wheel as usize];
        assert_eq!(w.name, "Cheese Wheel");
        assert!(w.solid && w.opaque);
        assert_eq!(w.bounce, 0.8);
        assert!(w.tex[0] >= FIRST_MOD_TILE);
        assert_eq!(reg.blocks[ore as usize].drop, slice);
        assert_eq!(reg.blocks[ore as usize].tex[1], crate::texture::T_STONE);
        let wand = &reg.items[(reg.lookup("cheese:cheese_wand").unwrap() - FIRST_ITEM) as usize];
        assert_eq!(wand.on_use, vec![Action::Launch(15.0), Action::Message("Wheee!"), Action::Give(slice, 2)]);
        assert_eq!(wand.tile, crate::texture::T_STICK);
        assert_eq!(reg.items[(slice - FIRST_ITEM) as usize].food, Some(3.0));
        assert!(reg.recipes.iter().any(|r| r.output == (wheel, 1) && r.inputs == vec![(slice, 4)]));
        assert_eq!(reg.ores.len(), 1);
        assert_eq!(reg.splashes, vec!["Now with cheese!".to_string()]);
        assert_eq!(reg.textures.len(), 2);
        // Pixel rows landed where expected.
        let (_, px) = reg.textures.iter().find(|(t, _)| *t == w.tex[0]).unwrap();
        assert_eq!(&px[..4], &[240, 200, 60, 255]);
        assert_eq!(&px[(16 + 1) * 4..(16 + 1) * 4 + 4], &[200, 160, 40, 255]);
    }

    #[test]
    fn disabled_mods_add_nothing_and_errors_have_line_numbers() {
        let reg = build(&[cheese()], &["cheese".into()]);
        assert!(!reg.mods[0].enabled);
        assert_eq!(reg.blocks.len(), NUM_BLOCKS as usize);

        let mut files = BTreeMap::new();
        files.insert("mod.txt".to_string(), b"[block rock]\nhardness = lots\ntexture = nope\n[recipe]\ninputs = unobtainium 2\noutput = rock\n".to_vec());
        let reg = build(&[ModSource { id: "bad".into(), files }], &[]);
        let errs = &reg.mods[0].errors;
        assert!(errs.iter().any(|e| e.contains("line 2") && e.contains("hardness")), "{errs:?}");
        assert!(errs.iter().any(|e| e.contains("line 3") && e.contains("unknown texture")), "{errs:?}");
        assert!(errs.iter().any(|e| e.contains("line 5") && e.contains("unobtainium")), "{errs:?}");
        // The block itself still exists with defaults.
        assert!(reg.lookup("bad:rock").is_some());
    }

    #[test]
    fn the_shipped_example_mod_loads_cleanly() {
        let sources = read_disk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("example-mods"));
        assert_eq!(sources.len(), 1);
        assert!(sources[0].files.contains_key("wheel_side.png"));
        let reg = build(&sources, &[]);
        let info = &reg.mods[0];
        assert!(info.errors.is_empty(), "example mod has errors: {:#?}", info.errors);
        assert_eq!(info.added, (5, 4, 7));
        assert_eq!(reg.ores.len(), 1);
        assert_eq!(reg.plants.len(), 1);
        assert_eq!(reg.splashes.len(), 3);
        // The PNG side texture decoded to real pixels.
        let wheel = &reg.blocks[reg.lookup("cheese:wheel").unwrap() as usize];
        let (_, px) = reg.textures.iter().find(|(t, _)| *t == wheel.tex[1]).unwrap();
        assert_eq!(px[3], 255);
        assert_eq!(wheel.bounce, 0.85);
    }

    #[test]
    fn packs_round_trip_and_reject_paths() {
        let mut m = cheese();
        m.files.insert("tex.png".into(), vec![1, 2, 3]);
        let packed = encode_pack(&[m.clone()]);
        assert_eq!(decode_pack(&packed).unwrap(), vec![m]);

        let mut evil = cheese();
        evil.files.insert("../../etc/passwd".into(), b"x".to_vec());
        let back = decode_pack(&encode_pack(&[evil])).unwrap();
        assert!(back[0].files.keys().all(|k| !k.contains('/')));
        assert!(decode_pack(&[255, 255, 255, 255]).is_err());
    }
}
