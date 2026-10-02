//! Data-driven mods. A mod is a folder in `mods/` containing a `mod.txt`
//! (plus optional 16x16 PNG textures). Mods add blocks, items, recipes,
//! textures, world generation and simple behaviours; they never run code,
//! so they're safe to share, and servers send theirs to joining players.
//! See MODDING.md for the file format.

use crate::block::build::{def, item, leak};
use crate::block::*;
use crate::noise::Rng;
use crate::texture::{base_texture, FIRST_MOD_TILE, TILE, TILES_PER_ROW};
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
                    if (lower.ends_with(".txt") || lower.ends_with(".png") || lower.ends_with(".rhai")) && f.path().is_file()
                        && let Ok(bytes) = std::fs::read(f.path())
                            && bytes.len() <= MAX_FILE {
                                files.insert(fname, bytes);
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

/// The active mods, packed for sending to a joining player. Scripts stay on
/// the host (only the host runs them), so they're left out.
pub fn active_pack() -> Vec<u8> {
    let without_scripts: Vec<ModSource> = ACTIVE
        .lock()
        .unwrap()
        .iter()
        .map(|m| ModSource { id: m.id.clone(), files: m.files.iter().filter(|(n, _)| !n.to_ascii_lowercase().ends_with(".rhai")).map(|(k, v)| (k.clone(), v.clone())).collect() })
        .collect();
    encode_pack(&without_scripts)
}

/// The active (enabled) mods' files, for starting their scripts.
pub fn active_sources() -> Vec<ModSource> {
    ACTIVE.lock().unwrap().clone()
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
    /// Resolve a mob name to a wire index for the `spawn` action: a base-game
    /// kind, or a mod mob in the registry being built (which may be this mod's
    /// own, so resolve against `self.reg` rather than the installed registry).
    fn spawn_kind(&self, modid: &str, name: &str) -> Option<u8> {
        let lower = name.trim().to_ascii_lowercase();
        if let Some(k) = crate::entity::MobKind::from_base_name_public(&lower) {
            return Some(k.index());
        }
        let keyed = format!("{modid}:{lower}");
        self.reg
            .mobs
            .iter()
            .position(|m| m.key == lower || m.key == keyed)
            .map(|i| crate::entity::BASE_MOBS.saturating_add(i.min(crate::entity::MAX_MOD_MOBS - 1) as u8))
    }

    fn resolve(&self, modid: &str, name: &str) -> Option<Id> {
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
        if self.next_tile >= TILES_PER_ROW * TILES_PER_ROW {
            errs.push(format!("line {line}: out of texture space ({} mod textures max)", TILES_PER_ROW * TILES_PER_ROW - FIRST_MOD_TILE));
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
                "spawn" => self.spawn_kind(modid, arg).map(Action::Spawn),
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
                        b.drop = reg.blocks.len() as Id;
                        reg.blocks.push(b);
                        info.added.0 += 1;
                        // Slabs and stairs get their other variants right after (filled in by `fill_block`).
                        for suffix in shape_variants(s.str("shape")) {
                            let v = def(leak(&format!("{key}{suffix}")), leak(&s.name), Model::Cube, true, true, [crate::texture::T_WHITE; 3], 1.0, 0, false, AIR, 0.0, 0);
                            reg.blocks.push(v);
                        }
                    }
                }
                "item" => {
                    if reg.items.len() + FIRST_ITEM as usize > Id::MAX as usize {
                        errs.push(format!("line {}: too many items across all mods", s.line));
                    } else if reg.lookup(&key).is_some() {
                        errs.push(format!("line {}: {} is defined twice", s.line, s.name));
                    } else {
                        reg.items.push(item(leak(&key), leak(&s.name), crate::texture::T_WHITE));
                        info.added.1 += 1;
                    }
                }
                "mob" if s.name.is_empty() || s.name.contains(':') => {
                    errs.push(format!("line {}: [mob ...] needs a simple name like [mob mouse]", s.line));
                }
                "mob" => {
                    if reg.mobs.len() >= crate::entity::MAX_MOD_MOBS {
                        errs.push(format!("line {}: too many mobs across all mods ({} max)", s.line, crate::entity::MAX_MOD_MOBS));
                    } else if reg.mobs.iter().any(|m| m.key == key) {
                        errs.push(format!("line {}: {} is defined twice", s.line, s.name));
                    } else {
                        // Reserve the slot now (deterministic order); fill it in pass 2.
                        reg.mobs.push(ModMob { key: key.clone(), name: s.name.clone(), tile: crate::texture::T_WHITE, half_width: 0.4, height: 0.9, max_health: 10.0, speed: 1.0, hostile: false, attack_damage: 0.0, attack_reach: 1.3, aggro_range: 16.0, attack_cooldown: 1.0, ranged_damage: 0.0, ranged_range: 16.0, projectile_speed: 24.0, projectile_appearance: ProjectileAppearance::default(), ranged_cooldown: 2.0, drop: None, template: crate::block::MobTemplate::Quadruped });
                    }
                }
                _ => {}
            }
        }
        for (fname, bytes) in m.files.iter().filter(|(n, _)| n.to_ascii_lowercase().ends_with(".rhai")) {
            info.scripts += 1;
            if let Err(e) = crate::scripting::check(&String::from_utf8_lossy(bytes)) {
                errs.push(format!("{fname}: {e}"));
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
            if let Some(px) = build_texture(&mut ctx, m, s, errs)
                && let Some(t) = ctx.alloc_tile(px, errs, s.line) {
                    ctx.textures.insert(format!("{}:{}", m.id, s.name.to_ascii_lowercase()), t);
                }
        }
    }
    for (mi, sections, errs) in parsed.iter_mut() {
        let m = &sources[*mi];
        for s in sections.iter() {
            match s.kind.as_str() {
                "block" => fill_block(&mut ctx, m, s, errs),
                "item" => fill_item(&mut ctx, m, s, errs),
                "mob" => fill_mob(&mut ctx, m, s, errs),
                "recipe" => if let Some(r) = parse_recipe(&ctx, m, s, errs) {
                    ctx.reg.recipes.push(r);
                    if let Some(info) = ctx.reg.mods.iter_mut().find(|i| i.id == m.id) {
                        info.added.2 += 1;
                    }
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

/// The extra blocks a `shape` needs, by key suffix (declared in pass 1, filled in by `fill_block`).
fn shape_variants(shape: Option<&str>) -> &'static [&'static str] {
    match shape.map(str::to_ascii_lowercase).as_deref() {
        Some("slab") => &["_top"],
        Some("stairs") => &["_east", "_south", "_west"],
        _ => &[],
    }
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
    fill_shape(ctx, m, s, id, errs);
    fill_furnace(ctx, m, s, id, errs);
}

/// `shape = slab` or `shape = stairs`: turn the block (and its variants) into that shape.
fn fill_shape(ctx: &mut Ctx, m: &ModSource, s: &Section, id: Id, errs: &mut Vec<String>) {
    let shape = s.str("shape").map(str::to_ascii_lowercase);
    let shapes: Vec<Shape> = match shape.as_deref() {
        None | Some("cube") => return,
        Some("slab") => vec![Shape::Slab { top: false }, Shape::Slab { top: true }],
        Some("stairs") => (0..4).map(|facing| Shape::Stairs { facing }).collect(),
        Some(other) => {
            errs.push(format!("line {}: shape should be slab or stairs, not {other}", s.line));
            return;
        }
    };
    let full = match s.str("full") {
        Some(v) => ctx.resolve(&m.id, v).filter(|&b| b < FIRST_ITEM).unwrap_or_else(|| {
            errs.push(format!("line {}: full: unknown block \"{v}\"", s.line));
            AIR
        }),
        None => AIR,
    };
    let base = ctx.reg.blocks[id as usize].clone();
    for (k, shape) in shapes.into_iter().enumerate() {
        let i = id as usize + k;
        let key = ctx.reg.blocks[i].key;
        let mut b = base.clone();
        b.key = key;
        b.model = Model::Shaped;
        b.shape = shape;
        b.solid = true;
        b.opaque = false;
        b.see_through = false;
        b.family = id;
        b.full = full;
        b.creative = k == 0 && base.creative;
        ctx.reg.blocks[i] = b;
    }
}

/// `smelts_into = <item>` and `burns_for = <seconds>` (blocks and items alike).
fn fill_furnace(ctx: &mut Ctx, m: &ModSource, s: &Section, id: Id, errs: &mut Vec<String>) {
    if let Some(v) = s.str("smelts_into") {
        match ctx.resolve(&m.id, v).filter(|&o| o != AIR) {
            Some(out) => ctx.reg.smelting.push((id, out)),
            None => errs.push(format!("line {}: smelts_into: unknown item \"{v}\"", s.line)),
        }
    }
    if s.get("burns_for").is_some() {
        let secs = num(s, "burns_for", 10.0f32, errs).clamp(0.5, 600.0);
        ctx.reg.fuels.push((id, secs));
    }
}

fn fill_item(ctx: &mut Ctx, m: &ModSource, s: &Section, errs: &mut Vec<String>) {
    let Some(id) = ctx.reg.lookup(&format!("{}:{}", m.id, s.name.to_ascii_lowercase())) else { return };
    if id < FIRST_ITEM {
        return;
    }
    let tile = s.get("texture").map(|(_, v, l)| ctx.texture(m, v, errs, *l)).unwrap_or(crate::texture::T_WHITE);
    let on_use = s.get("on_use").map(|(_, v, l)| ctx.actions(&m.id, v, errs, *l)).unwrap_or_default();
    let food = s.get("food").map(|_| num(s, "food", 1.0f32, errs).clamp(0.0, 20.0));
    let armor = match s.str("armor").map(str::to_ascii_lowercase).as_deref() {
        None => None,
        Some(slot) => match ["helmet", "chestplate", "leggings", "boots"].iter().position(|x| *x == slot) {
            Some(slot) => {
                let looks = s.str("looks_like").unwrap_or("iron").to_ascii_lowercase();
                let looks_like = ["wool", "iron", "gold", "diamond"].iter().position(|x| *x == looks).unwrap_or_else(|| {
                    errs.push(format!("line {}: looks_like should be wool, iron, gold or diamond, not {looks}", s.line));
                    1
                }) as u8;
                Some(ModArmor { slot: slot as u8, points: num(s, "armor_points", 2u8, errs).clamp(1, 10), looks_like })
            }
            None => {
                errs.push(format!("line {}: armor should be helmet, chestplate, leggings or boots, not {slot}", s.line));
                None
            }
        },
    };
    let durability = match (s.get("durability"), armor) {
        // At most 16000, so Unbreaking III (four times as many uses) still fits.
        (Some(_), _) => Some(num(s, "durability", 100u16, errs).clamp(1, 16000)),
        (None, Some(_)) => Some(200),
        _ => None,
    };
    let repair = match s.str("repair") {
        Some(v) => ctx.resolve(&m.id, v).unwrap_or_else(|| {
            errs.push(format!("line {}: repair: unknown item \"{v}\"", s.line));
            AIR
        }),
        None => AIR,
    };
    fill_furnace(ctx, m, s, id, errs);
    let it = &mut ctx.reg.items[(id - FIRST_ITEM) as usize];
    it.durability = durability;
    it.armor = armor;
    it.repair = repair;
    if let Some(n) = s.str("name") {
        it.name = leak(n);
    }
    it.tile = tile;
    // Things that wear out don't stack.
    it.stack = if durability.is_some() { 1 } else { num(s, "stack", 64u8, errs).clamp(1, 64) };
    it.pick_tier = num(s, "pickaxe", 0u8, errs).min(4);
    it.damage = num(s, "damage", 1.0f32, errs).clamp(0.0, 40.0);
    it.food = food;
    it.on_use = on_use;
    it.consume = flag(s, "consume", true, errs);
}

fn fill_mob(ctx: &mut Ctx, m: &ModSource, s: &Section, errs: &mut Vec<String>) {
    let key = format!("{}:{}", m.id, s.name.to_ascii_lowercase());
    let Some(i) = ctx.reg.mobs.iter().position(|mob| mob.key == key) else { return };
    let tile = s.get("texture").map(|(_, v, l)| ctx.texture(m, v, errs, *l)).unwrap_or(crate::texture::T_WHITE);
    let template = match s.str("template").or_else(|| s.str("model")) {
        None => crate::block::MobTemplate::Quadruped,
        Some(v) => crate::block::MobTemplate::from_name(v).unwrap_or_else(|| {
            errs.push(format!("line {}: template should be quadruped, biped, blob or bird, not \"{v}\"", s.line));
            crate::block::MobTemplate::Quadruped
        }),
    };
    // A single `size` sets both dimensions; `width`/`height` override.
    let size = num(s, "size", 0.9f32, errs);
    let half_width = (num(s, "width", size, errs) * 0.5).clamp(0.1, 4.0);
    let height = num(s, "height", size, errs).clamp(0.2, 8.0);
    let max_health = num(s, "health", 10.0f32, errs).clamp(1.0, 1000.0);
    let speed = num(s, "speed", 1.0f32, errs).clamp(0.1, 4.0);
    let hostile = flag(s, "hostile", false, errs);
    // Melee attack parameters. All clamped so a malformed/adversarial mod cannot
    // wedge or exploit the sim. BACKWARD COMPAT: attack_damage defaults to 0.0,
    // so an existing [mob] with hostile=true but no attack fields keeps the exact
    // v1 behavior (counts toward the monster cap but never attacks). A mob attacks
    // only when it is hostile AND attack_damage > 0.0.
    let attack_damage = num(s, "attack_damage", 0.0f32, errs).clamp(0.0, 50.0);
    let attack_reach = num(s, "attack_reach", 1.3f32, errs).clamp(0.5, 4.0);
    let aggro_range = num(s, "aggro_range", 16.0f32, errs).clamp(1.0, 48.0);
    // The 0.25 minimum is essential: it prevents a zero-cooldown DPS exploit.
    let attack_cooldown = num(s, "attack_cooldown", 1.0f32, errs).clamp(0.25, 10.0);
    // Ranged attack parameters, all clamped like the melee ones. BACKWARD COMPAT:
    // ranged_damage defaults to 0.0, so an existing [mob] section is unchanged and
    // never fires. A mob shoots only when it is hostile AND ranged_damage > 0.0.
    // The 0.5 cooldown floor prevents a zero-cooldown projectile-spam exploit, and
    // the speed/range clamps bound what an adversarial mod can do.
    let ranged_damage = num(s, "ranged_damage", 0.0f32, errs).clamp(0.0, 30.0);
    let ranged_range = num(s, "ranged_range", 16.0f32, errs).clamp(1.0, 48.0);
    let projectile_speed = num(s, "projectile_speed", 24.0f32, errs).clamp(8.0, 48.0);
    let projectile_model = match s.get("projectile_model") {
        None => ProjectileModel::Arrow,
        Some((_, value, line)) => match value.trim().to_ascii_lowercase().as_str() {
            "arrow" => ProjectileModel::Arrow,
            "billboard" => ProjectileModel::Billboard,
            "cube" => ProjectileModel::Cube,
            _ => {
                errs.push(format!("line {line}: projectile_model should be arrow, billboard or cube, not \"{value}\""));
                ProjectileModel::Arrow
            }
        },
    };
    let projectile_tile = s.get("projectile_texture").map(|(_, value, line)| ctx.texture(m, value, errs, *line));
    let projectile_scale = num(s, "projectile_scale", 1.0f32, errs).clamp(0.25, 4.0);
    let projectile_appearance = ProjectileAppearance { model: projectile_model, tile: projectile_tile, scale: projectile_scale };
    let ranged_cooldown = num(s, "ranged_cooldown", 2.0f32, errs).clamp(0.5, 10.0);
    let drop = match s.get("drops") {
        None => None,
        Some((_, v, l)) => parse_stack(ctx, &m.id, v, errs, *l),
    };
    if let Some(name) = s.str("name") {
        ctx.reg.mobs[i].name = name.to_string();
    }
    let mob = &mut ctx.reg.mobs[i];
    mob.tile = tile;
    mob.template = template;
    mob.half_width = half_width;
    mob.height = height;
    mob.max_health = max_health;
    mob.speed = speed;
    mob.hostile = hostile;
    mob.attack_damage = attack_damage;
    mob.attack_reach = attack_reach;
    mob.aggro_range = aggro_range;
    mob.attack_cooldown = attack_cooldown;
    mob.ranged_damage = ranged_damage;
    mob.ranged_range = ranged_range;
    mob.projectile_speed = projectile_speed;
    mob.projectile_appearance = projectile_appearance;
    mob.ranged_cooldown = ranged_cooldown;
    mob.drop = drop;
}

fn parse_stack(ctx: &Ctx, modid: &str, v: &str, errs: &mut Vec<String>, line: usize) -> Option<(Id, u8)> {
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
    let ins: Vec<(Id, u8)> = inputs.split(',').filter(|p| !p.trim().is_empty()).filter_map(|p| parse_stack(ctx, &m.id, p, errs, *il)).collect();
    let out = parse_stack(ctx, &m.id, output, errs, *ol)?;
    if ins.is_empty() || ins.len() > 6 {
        errs.push(format!("line {il}: a recipe needs 1 to 6 inputs"));
        return None;
    }
    Some(Recipe { inputs: ins, output: out })
}

/// A process-wide lock shared by every test that cares about which registry is
/// globally installed. The gear/furnace end-to-end tests install a mod
/// registry for a moment (see `with_mods`); any test that asserts the *base*
/// registry's exact shape takes the same lock so the two never overlap.
#[cfg(test)]
pub(crate) fn registry_test_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::Mutex;
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Test helper: build a registry from inline `mod.txt` sources, install it
/// globally, run `body`, then put the base registry back. Serialised across
/// the whole test binary (via `registry_test_lock`) so the brief window where
/// `reg()` holds mod content never overlaps a test that reads the registry.
/// Returns whatever `body` returns.
#[cfg(test)]
pub(crate) fn with_mods<T>(sources: &[(&str, &str)], body: impl FnOnce(&Registry) -> T) -> T {
    let srcs: Vec<ModSource> = sources
        .iter()
        .map(|(id, text)| {
            let mut files = BTreeMap::new();
            files.insert("mod.txt".to_string(), text.as_bytes().to_vec());
            ModSource { id: (*id).to_string(), files }
        })
        .collect();
    let reg = build(&srcs, &[]);
    // Hold the lock for the whole installed window, and always restore the base
    // registry afterwards (even if `body` panics).
    let _guard = registry_test_lock();
    install(reg);
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(crate::block::reg())));
    install(Registry::base());
    match out {
        Ok(v) => v,
        Err(e) => std::panic::resume_unwind(e),
    }
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
    fn mods_add_gear_shapes_and_cooking() {
        let text = r#"
[block marble]
texture = stone

[block marble_slab]
texture = stone
shape = slab
full = marble

[block marble_stairs]
texture = stone
shape = stairs
burns_for = 3

[item ruby]
smelts_into = gold

[item ruby_sword]
damage = 7
durability = 900
repair = ruby

[item ruby_helmet]
armor = helmet
armor_points = 4
looks_like = gold

[item bad]
armor = hat
"#;
        let mut files = BTreeMap::new();
        files.insert("mod.txt".to_string(), text.as_bytes().to_vec());
        let reg = build(&[ModSource { id: "gems".into(), files }], &[]);
        let errs = &reg.mods[0].errors;
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].contains("hat"));
        // Slabs and stairs get their variants, in order, with their shapes.
        let marble = reg.lookup("gems:marble").unwrap();
        let slab = reg.lookup("gems:marble_slab").unwrap();
        assert_eq!(reg.lookup("gems:marble_slab_top"), Some(slab + 1));
        let top = &reg.blocks[slab as usize + 1];
        assert_eq!((top.shape, top.family, top.full, top.creative), (Shape::Slab { top: true }, slab, marble, false));
        let stairs = reg.lookup("gems:marble_stairs").unwrap();
        assert_eq!(reg.lookup("gems:marble_stairs_west"), Some(stairs + 3));
        assert_eq!(reg.blocks[stairs as usize + 2].shape, Shape::Stairs { facing: 2 });
        assert_eq!(reg.blocks[stairs as usize].model, Model::Shaped);
        // Furnace recipes and fuels.
        let ruby = reg.lookup("gems:ruby").unwrap();
        assert!(reg.smelting.contains(&(ruby, GOLD_INGOT)));
        assert!(reg.fuels.contains(&(stairs, 3.0)));
        // Durability, repair and armour.
        let sword = &reg.items[(reg.lookup("gems:ruby_sword").unwrap() - FIRST_ITEM) as usize];
        assert_eq!((sword.durability, sword.stack, sword.repair), (Some(900), 1, ruby));
        let helmet = &reg.items[(reg.lookup("gems:ruby_helmet").unwrap() - FIRST_ITEM) as usize];
        assert_eq!(helmet.armor, Some(ModArmor { slot: 0, points: 4, looks_like: 2 }));
        assert_eq!(helmet.durability, Some(200));
    }

    #[test]
    fn mods_define_new_mob_types() {
        let text = r#"
[item nib]
texture = stick

[mob mouse]
name = Tiny Mouse
texture = stone
template = quadruped
size = 0.4
health = 6
speed = 1.5
hostile = false
drops = nib 2

[mob brute]
texture = stone
template = biped
width = 0.8
height = 2.1
health = 40
hostile = true

[mob bad]
template = dragon
"#;
        let mut files = BTreeMap::new();
        files.insert("mod.txt".to_string(), text.as_bytes().to_vec());
        let reg = build(&[ModSource { id: "zoo".into(), files }], &[]);
        // The only error is the unknown template.
        let errs = &reg.mods[0].errors;
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].contains("template"));
        // Two mobs, in declaration order.
        assert_eq!(reg.mobs.len(), 3);
        let mouse = &reg.mobs[0];
        assert_eq!(mouse.key, "zoo:mouse");
        assert_eq!(mouse.name, "Tiny Mouse");
        assert_eq!(mouse.template, crate::block::MobTemplate::Quadruped);
        assert_eq!(mouse.max_health, 6.0);
        assert_eq!(mouse.speed, 1.5);
        assert!(!mouse.hostile);
        assert_eq!(mouse.half_width, 0.2);
        assert_eq!(mouse.height, 0.4);
        let nib = reg.lookup("zoo:nib").unwrap();
        assert_eq!(mouse.drop, Some((nib, 2)));
        // A base-atlas texture name resolves to that tile (no mod tile needed).
        assert_eq!(mouse.tile, crate::texture::T_STONE);
        let brute = &reg.mobs[1];
        assert_eq!(brute.template, crate::block::MobTemplate::Biped);
        assert_eq!((brute.half_width, brute.height), (0.4, 2.1));
        assert!(brute.hostile);
        // The one with an unknown template still exists, with the default shape.
        assert_eq!(reg.mobs[2].template, crate::block::MobTemplate::Quadruped);
    }

    #[test]
    fn mob_attack_fields_parse_and_clamp() {
        let text = r#"
[mob fierce]
texture = stone
template = biped
hostile = true
attack_damage = 999
attack_reach = 99
aggro_range = 999
attack_cooldown = 0.01
ranged_damage = 999
ranged_range = 999
projectile_speed = 999
ranged_cooldown = 0.01

[mob gentle]
texture = stone
template = quadruped
hostile = true
attack_reach = 0.1
aggro_range = 0.1
ranged_range = 0.1
projectile_speed = 0.1
ranged_cooldown = 99
"#;
        let mut files = BTreeMap::new();
        files.insert("mod.txt".to_string(), text.as_bytes().to_vec());
        let reg = build(&[ModSource { id: "zoo".into(), files }], &[]);
        assert!(reg.mods[0].errors.is_empty(), "{:?}", reg.mods[0].errors);
        let fierce = &reg.mobs[0];
        // Over-range values clamp down to their maxima.
        assert_eq!(fierce.attack_damage, 50.0, "damage clamps to 50");
        assert_eq!(fierce.attack_reach, 4.0, "reach clamps to 4");
        assert_eq!(fierce.aggro_range, 48.0, "aggro clamps to 48");
        // The critical anti-DPS-exploit clamp: cooldown has a 0.25 minimum.
        assert_eq!(fierce.attack_cooldown, 0.25, "cooldown clamps up to 0.25");
        // Ranged fields clamp down to their maxima / up to their minima too.
        assert_eq!(fierce.ranged_damage, 30.0, "ranged damage clamps to 30");
        assert_eq!(fierce.ranged_range, 48.0, "ranged range clamps to 48");
        assert_eq!(fierce.projectile_speed, 48.0, "projectile speed clamps to 48");
        // The anti-spam clamp: ranged cooldown has a 0.5 minimum.
        assert_eq!(fierce.ranged_cooldown, 0.5, "ranged cooldown clamps up to 0.5");
        // Under-range reach/aggro clamp up to their minima.
        let gentle = &reg.mobs[1];
        assert_eq!(gentle.attack_reach, 0.5, "reach clamps up to 0.5");
        assert_eq!(gentle.aggro_range, 1.0, "aggro clamps up to 1.0");
        // Under-range ranged range/speed clamp up; over-range cooldown clamps down.
        assert_eq!(gentle.ranged_range, 1.0, "ranged range clamps up to 1.0");
        assert_eq!(gentle.projectile_speed, 8.0, "projectile speed clamps up to 8.0");
        assert_eq!(gentle.ranged_cooldown, 10.0, "ranged cooldown clamps down to 10.0");
        // Backward compat: no attack fields -> attack_damage defaults to 0.0.
        assert_eq!(gentle.attack_damage, 0.0, "default attack_damage is 0.0 (non-attacking)");
        assert_eq!(gentle.attack_cooldown, 1.0, "default cooldown is 1.0");
    }

    #[test]
    fn mob_ranged_fields_default_off() {
        // A mob with no ranged fields at all keeps the backward-compatible
        // defaults: ranged OFF (ranged_damage == 0.0) and the documented
        // speed/range/cooldown defaults.
        let text = r#"
[mob plain]
texture = stone
hostile = true
attack_damage = 5
"#;
        let mut files = BTreeMap::new();
        files.insert("mod.txt".to_string(), text.as_bytes().to_vec());
        let reg = build(&[ModSource { id: "zoo".into(), files }], &[]);
        assert!(reg.mods[0].errors.is_empty(), "{:?}", reg.mods[0].errors);
        let plain = &reg.mobs[0];
        assert_eq!(plain.ranged_damage, 0.0, "ranged is OFF by default");
        assert_eq!(plain.ranged_range, 16.0, "default ranged range is 16.0");
        assert_eq!(plain.projectile_speed, 24.0, "default projectile speed is 24.0");
        assert_eq!(plain.ranged_cooldown, 2.0, "default ranged cooldown is 2.0");
    }

    #[test]
    fn projectile_appearance_parser_defaults_clamps_and_reports_errors() {
        let art = r#"
[texture bolt]
noise = 200,100,50

[mob local]
projectile_model = billboard
projectile_texture = bolt
projectile_scale = 9
"#;
        let mobs = r#"
[mob plain]

[mob qualified]
projectile_model = cube
projectile_texture = art:bolt
projectile_scale = 0.1

[mob base]
projectile_model = arrow
projectile_texture = stone
projectile_scale = 2

[mob bad]
projectile_model = pyramid
projectile_texture = missing
"#;
        let source = |id: &str, text: &str| {
            let mut files = BTreeMap::new();
            files.insert("mod.txt".to_string(), text.as_bytes().to_vec());
            ModSource { id: id.to_string(), files }
        };
        let reg = build(&[source("art", art), source("mobs", mobs)], &[]);
        let tile = reg.mobs[0].projectile_appearance.tile.expect("local texture");
        assert_eq!(reg.mobs[0].projectile_appearance, ProjectileAppearance { model: ProjectileModel::Billboard, tile: Some(tile), scale: 4.0 });
        assert_eq!(reg.mobs[1].projectile_appearance, ProjectileAppearance::default());
        assert_eq!(reg.mobs[2].projectile_appearance, ProjectileAppearance { model: ProjectileModel::Cube, tile: Some(tile), scale: 0.25 });
        assert_eq!(reg.mobs[3].projectile_appearance, ProjectileAppearance { model: ProjectileModel::Arrow, tile: Some(crate::texture::T_STONE), scale: 2.0 });
        assert_eq!(reg.mobs[4].projectile_appearance.model, ProjectileModel::Arrow);
        assert_eq!(reg.mobs[4].projectile_appearance.tile, Some(crate::texture::T_WHITE));
        let errors = &reg.mods[1].errors;
        assert!(errors.iter().any(|e| e.contains("line") && e.contains("projectile_model")), "{errors:?}");
        assert!(errors.iter().any(|e| e.contains("line") && e.contains("unknown texture")), "{errors:?}");
    }

    #[test]
    fn a_mob_section_needs_a_name() {
        let mut files = BTreeMap::new();
        files.insert("mod.txt".to_string(), b"[mob]\nhealth = 5\n".to_vec());
        let reg = build(&[ModSource { id: "z".into(), files }], &[]);
        assert!(reg.mobs.is_empty());
        assert!(reg.mods[0].errors.iter().any(|e| e.contains("[mob ...] needs a simple name")));
    }

    #[test]
    fn mod_mob_resolves_by_name_once_installed() {
        use crate::entity::MobKind;
        let src = "[mob mouse]\ntexture = stone\nhealth = 6\n";
        with_mods(&[("zoo", src)], |reg| {
            assert_eq!(reg.mobs.len(), 1);
            // Resolves by full key, bare name, and both are the same Modded kind.
            let k = MobKind::from_name("zoo:mouse").expect("key resolves");
            assert_eq!(k, MobKind::Modded(0));
            assert_eq!(MobKind::from_name("mouse"), Some(k));
            // It carries the def's stats, and renders from a (non-empty) template.
            assert_eq!(k.max_health(), 6.0);
            assert_eq!(k.name(), "mouse");
            assert!(!crate::entity::modded_parts(&reg.mobs[0]).is_empty());
            // Round-trips through the wire index.
            assert_eq!(MobKind::from_index(k.index()), Some(k));
            // One past the end is unknown, not a panic.
            assert_eq!(MobKind::from_index(k.index() + 1), None);
        });
    }

    #[test]
    fn hundreds_of_mod_blocks_and_items_fit() {
        // Far more than the old one-byte ids allowed (76 blocks, 138 items).
        let mut text = String::new();
        for i in 0..600 {
            text.push_str(&format!("[block b{i}]\n[item i{i}]\n"));
        }
        text.push_str("[recipe]\ninputs = b599 2\noutput = i599\n");
        let mut files = BTreeMap::new();
        files.insert("mod.txt".to_string(), text.into_bytes());
        let reg = build(&[ModSource { id: "big".into(), files }], &[]);
        assert!(reg.mods[0].errors.is_empty(), "{:?}", &reg.mods[0].errors[..3.min(reg.mods[0].errors.len())]);
        assert_eq!(reg.mods[0].added.0, 600);
        let last_block = reg.lookup("big:b599").unwrap();
        let last_item = reg.lookup("big:i599").unwrap();
        assert_eq!(last_block, NUM_BLOCKS + 599);
        assert_eq!(last_item, FIRST_MOD_ITEM + 599);
        assert_eq!(reg.key_of(last_item), "big:i599");
        assert!(reg.recipes.iter().any(|r| r.output == (last_item, 1) && r.inputs == vec![(last_block, 2)]));
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
        let sources: Vec<ModSource> = read_disk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("example-mods")).into_iter().filter(|m| m.id == "cheese").collect();
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
