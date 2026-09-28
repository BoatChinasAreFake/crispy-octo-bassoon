//! Code mods: sandboxed Rhai scripts (`*.rhai` files in a mod folder).
//!
//! Scripts define event functions (`on_chat`, `on_block_break`, `on_tick`, ...)
//! and call a game API (`set_block`, `give`, `teleport`, ...). They run only
//! where the world lives: single player, the host, or a dedicated server.
//! Joining players never run scripts.
//!
//! Sandboxing: Rhai has no file, network or OS access; we also disable
//! `eval`, block module imports, and cap operations, recursion, and
//! string/array/map sizes, so a buggy or hostile script can't hang the game.
//!
//! API calls that change the world are queued and applied right after the
//! event function returns. See SCRIPTING.md.

use crate::block::*;
use crate::game::Game;
use crate::mods::ModSource;
use crate::noise::Rng;
use macroquad::math::Vec3;
use rhai::{Array, Dynamic, Engine, EvalAltResult, ImmutableString, Position, Scope, AST, FLOAT, INT};
use std::cell::RefCell;
use std::collections::HashMap;

/// Per-call operation budget (a tight infinite loop hits this in a few ms).
const MAX_OPS: u64 = 500_000;
/// Scripts are switched off after this many runtime errors.
const MAX_ERRORS: u32 = 25;
const MAX_CMDS: usize = 20_000;
const MAX_FILL: i64 = 32_768;
pub const TICK: f32 = 0.05;

/// A world change requested by a script.
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    SetBlock(i32, i32, i32, u8),
    Message(String, String),
    Broadcast(String),
    Give(String, u8, u8),
    Take(String, u8, u8),
    Heal(String, f32),
    Damage(String, f32),
    Teleport(String, Vec3),
    Launch(String, f32),
    Explode(Vec3, f32),
    Spawn(u8, Vec3),
    SetTime(f32),
    Sound(crate::sound::Sfx, Vec3),
}

struct Ctx {
    game: *const Game,
    cmds: Vec<Cmd>,
    vars: HashMap<String, Dynamic>,
    log: Vec<String>,
    rng: Rng,
}

thread_local! {
    static CTX: RefCell<Option<Ctx>> = const { RefCell::new(None) };
}

type Res<T> = Result<T, Box<EvalAltResult>>;

fn err<T>(msg: impl Into<String>) -> Res<T> {
    Err(Box::new(EvalAltResult::ErrorRuntime(Dynamic::from(msg.into()), Position::NONE)))
}

/// Read-only access to the game during a script call.
fn with_game<R>(f: impl FnOnce(&Game) -> R) -> Res<R> {
    CTX.with(|c| match c.borrow().as_ref() {
        // SAFETY: the pointer is set from a live `&Game` for the duration of the
        // call (see `ScriptHost::call`), and scripts can't mutate the game directly.
        Some(ctx) => Ok(f(unsafe { &*ctx.game })),
        None => err("game API used outside an event"),
    })
}

fn with_ctx<R>(f: impl FnOnce(&mut Ctx) -> R) -> Res<R> {
    CTX.with(|c| match c.borrow_mut().as_mut() {
        Some(ctx) => Ok(f(ctx)),
        None => err("game API used outside an event"),
    })
}

fn push(cmd: Cmd) -> Res<()> {
    with_ctx(|ctx| {
        if ctx.cmds.len() < MAX_CMDS {
            ctx.cmds.push(cmd);
            true
        } else {
            false
        }
    })
    .and_then(|ok| if ok { Ok(()) } else { err("too many world changes in one event") })
}

fn num(d: &Dynamic) -> Res<f64> {
    if let Some(i) = d.clone().try_cast::<INT>() {
        return Ok(i as f64);
    }
    if let Some(f) = d.clone().try_cast::<FLOAT>() {
        return Ok(f);
    }
    err(format!("expected a number, got {}", d.type_name()))
}

fn coord(d: &Dynamic) -> Res<i32> {
    let v = num(d)?.floor();
    if v.abs() > 30_000_000.0 { err("coordinate out of range") } else { Ok(v as i32) }
}

fn lookup(name: &str) -> Res<u8> {
    let r = reg();
    let n = name.trim().to_ascii_lowercase();
    r.lookup(&n).or_else(|| r.blocks.iter().position(|b| b.key.ends_with(&format!(":{n}"))).map(|i| i as u8)).map(Ok).unwrap_or_else(|| err(format!("unknown block or item \"{name}\"")))
}

fn block_id(name: &str) -> Res<u8> {
    let id = lookup(name)?;
    if id >= FIRST_ITEM { err(format!("\"{name}\" is an item, not a block")) } else { Ok(id) }
}

fn count(d: &Dynamic) -> Res<u8> {
    Ok(num(d)?.clamp(1.0, 64.0) as u8)
}

fn make_engine() -> Engine {
    let mut e = Engine::new();
    e.set_max_operations(MAX_OPS);
    e.set_max_call_levels(48);
    e.set_max_expr_depths(64, 32);
    e.set_max_string_size(16 * 1024);
    e.set_max_array_size(16 * 1024);
    e.set_max_map_size(4 * 1024);
    e.disable_symbol("eval");
    e.set_module_resolver(rhai::module_resolvers::DummyModuleResolver::new());
    e.on_print(|s| {
        let _ = with_ctx(|c| c.log.push(s.to_string()));
    });
    e.on_debug(|s, _, _| {
        let _ = with_ctx(|c| c.log.push(s.to_string()));
    });
    register_api(&mut e);
    e
}

fn register_api(e: &mut Engine) {
    // ---- reading the world
    e.register_fn("get_block", |x: Dynamic, y: Dynamic, z: Dynamic| -> Res<ImmutableString> {
        let (x, y, z) = (coord(&x)?, coord(&y)?, coord(&z)?);
        with_game(|g| reg().key_of(g.world.get(x, y, z)).into())
    });
    e.register_fn("surface_y", |x: Dynamic, z: Dynamic| -> Res<INT> {
        let (x, z) = (coord(&x)?, coord(&z)?);
        with_game(|g| g.world.surface_y(x, z) as INT)
    });
    e.register_fn("block_exists", |name: &str| lookup(name).is_ok());
    e.register_fn("players", || -> Res<Array> { with_game(|g| g.player_names().into_iter().map(Dynamic::from).collect()) });
    e.register_fn("player_pos", |name: &str| -> Res<Dynamic> {
        with_game(|g| match g.player_position(name) {
            Some(p) => Dynamic::from(vec![Dynamic::from(p.x as FLOAT), Dynamic::from(p.y as FLOAT), Dynamic::from(p.z as FLOAT)]),
            None => Dynamic::UNIT,
        })
    });
    e.register_fn("player_health", |name: &str| -> Res<FLOAT> {
        with_game(|g| if g.is_local_player(name) { g.player.health as FLOAT } else { -1.0 })
    });
    e.register_fn("held_item", |name: &str| -> Res<ImmutableString> {
        with_game(|g| if g.is_local_player(name) { reg().key_of(g.inv.held()).into() } else { "".into() })
    });
    e.register_fn("count_item", |name: &str, item: &str| -> Res<INT> {
        let id = lookup(item)?;
        with_game(|g| if g.is_local_player(name) { g.inv.count(id) as INT } else { 0 })
    });
    e.register_fn("time", || -> Res<FLOAT> { with_game(|g| g.time as FLOAT) });
    e.register_fn("is_night", || -> Res<bool> { with_game(|g| g.is_night()) });
    e.register_fn("mobs_near", |x: Dynamic, y: Dynamic, z: Dynamic, r: Dynamic| -> Res<INT> {
        let p = Vec3::new(num(&x)? as f32, num(&y)? as f32, num(&z)? as f32);
        let r = num(&r)? as f32;
        with_game(|g| g.mobs.iter().filter(|m| m.body.pos.distance(p) <= r).count() as INT)
    });
    e.register_fn("random", || -> Res<FLOAT> { with_ctx(|c| c.rng.f32() as FLOAT) });
    e.register_fn("random_int", |lo: INT, hi: INT| -> Res<INT> {
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        with_ctx(|c| lo + (c.rng.next_u64() % ((hi - lo) as u64 + 1)) as INT)
    });

    // ---- per-mod memory (lives until the world is closed)
    e.register_fn("get_var", |k: &str| -> Res<Dynamic> { with_ctx(|c| c.vars.get(k).cloned().unwrap_or(Dynamic::UNIT)) });
    e.register_fn("set_var", |k: &str, v: Dynamic| -> Res<()> {
        with_ctx(|c| {
            if c.vars.len() < 10_000 || c.vars.contains_key(k) {
                c.vars.insert(k.to_string(), v);
            }
        })
    });

    // ---- changing the world (applied when the event returns)
    e.register_fn("set_block", |x: Dynamic, y: Dynamic, z: Dynamic, name: &str| -> Res<()> {
        push(Cmd::SetBlock(coord(&x)?, coord(&y)?, coord(&z)?, block_id(name)?))
    });
    e.register_fn(
        "fill",
        |x1: Dynamic, y1: Dynamic, z1: Dynamic, x2: Dynamic, y2: Dynamic, z2: Dynamic, name: &str| -> Res<INT> {
            let id = block_id(name)?;
            let (a, b) = ((coord(&x1)?, coord(&y1)?, coord(&z1)?), (coord(&x2)?, coord(&y2)?, coord(&z2)?));
            let (lx, hx, ly, hy, lz, hz) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1).max(0), a.1.max(b.1).min(127), a.2.min(b.2), a.2.max(b.2));
            let volume = (hx - lx + 1) as i64 * (hy - ly + 1).max(0) as i64 * (hz - lz + 1) as i64;
            if volume > MAX_FILL {
                return err(format!("fill is too big ({volume} blocks, max {MAX_FILL})"));
            }
            for y in ly..=hy {
                for z in lz..=hz {
                    for x in lx..=hx {
                        push(Cmd::SetBlock(x, y, z, id))?;
                    }
                }
            }
            Ok(volume as INT)
        },
    );
    e.register_fn("message", |player: &str, text: &str| push(Cmd::Message(player.into(), text.chars().take(300).collect())));
    e.register_fn("broadcast", |text: &str| push(Cmd::Broadcast(text.chars().take(300).collect())));
    e.register_fn("give", |player: &str, item: &str, n: Dynamic| -> Res<()> { push(Cmd::Give(player.into(), lookup(item)?, count(&n)?)) });
    e.register_fn("give", |player: &str, item: &str| -> Res<()> { push(Cmd::Give(player.into(), lookup(item)?, 1)) });
    e.register_fn("take", |player: &str, item: &str, n: Dynamic| -> Res<()> { push(Cmd::Take(player.into(), lookup(item)?, count(&n)?)) });
    e.register_fn("heal", |player: &str, n: Dynamic| -> Res<()> { push(Cmd::Heal(player.into(), num(&n)?.clamp(0.0, 20.0) as f32)) });
    e.register_fn("damage", |player: &str, n: Dynamic| -> Res<()> { push(Cmd::Damage(player.into(), num(&n)?.clamp(0.0, 100.0) as f32)) });
    e.register_fn("teleport", |player: &str, x: Dynamic, y: Dynamic, z: Dynamic| -> Res<()> {
        let p = Vec3::new(num(&x)? as f32, (num(&y)? as f32).clamp(-10.0, 400.0), num(&z)? as f32);
        push(Cmd::Teleport(player.into(), p))
    });
    e.register_fn("launch", |player: &str, v: Dynamic| -> Res<()> { push(Cmd::Launch(player.into(), num(&v)?.clamp(-60.0, 60.0) as f32)) });
    e.register_fn("explode", |x: Dynamic, y: Dynamic, z: Dynamic, r: Dynamic| -> Res<()> {
        push(Cmd::Explode(Vec3::new(num(&x)? as f32, num(&y)? as f32, num(&z)? as f32), num(&r)?.clamp(0.5, 8.0) as f32))
    });
    e.register_fn("spawn_mob", |kind: &str, x: Dynamic, y: Dynamic, z: Dynamic| -> Res<()> {
        let k = match kind.to_ascii_lowercase().as_str() {
            "oinker" | "pig" => 0,
            "hisser" => 1,
            "groaner" | "zombie" => 2,
            other => return err(format!("unknown mob \"{other}\" (oinker, hisser, groaner)")),
        };
        push(Cmd::Spawn(k, Vec3::new(num(&x)? as f32, num(&y)? as f32, num(&z)? as f32)))
    });
    e.register_fn("set_time", |t: Dynamic| -> Res<()> { push(Cmd::SetTime(num(&t)?.rem_euclid(1.0) as f32)) });
    e.register_fn("play_sound", |name: &str, x: Dynamic, y: Dynamic, z: Dynamic| -> Res<()> {
        use crate::sound::{Mat, Sfx};
        let s = match name {
            "explode" => Sfx::Explode,
            "hiss" => Sfx::Hiss,
            "oink" => Sfx::Oink,
            "groan" => Sfx::Groan,
            "pop" => Sfx::Pop,
            "click" => Sfx::Click,
            "splash" => Sfx::Splash,
            "craft" => Sfx::Craft,
            "hurt" => Sfx::Hurt,
            "eat" => Sfx::Eat,
            "break" => Sfx::Break(Mat::Stone),
            "glass" => Sfx::Break(Mat::Glass),
            other => return err(format!("unknown sound \"{other}\"")),
        };
        push(Cmd::Sound(s, Vec3::new(num(&x)? as f32, num(&y)? as f32, num(&z)? as f32)))
    });
    e.register_fn("log", |text: &str| {
        let _ = with_ctx(|c| c.log.push(text.to_string()));
    });
}

/// Check a script compiles (used by the mod loader to report errors early).
pub fn check(source: &str) -> Result<(), String> {
    make_engine().compile(source).map(|_| ()).map_err(|e| e.to_string())
}

struct ScriptMod {
    id: String,
    ast: AST,
    /// Event functions this mod defines: name -> parameter count.
    fns: HashMap<String, usize>,
    vars: HashMap<String, Dynamic>,
    errors: u32,
    disabled: bool,
}

#[derive(Default)]
pub struct HookResult {
    pub cmds: Vec<Cmd>,
    /// False if any script returned `false` (cancel the default behaviour).
    pub allow: bool,
    /// Some script defines this event.
    pub handled: bool,
    pub log: Vec<String>,
    pub errors: Vec<String>,
}

pub struct ScriptHost {
    engine: Engine,
    mods: Vec<ScriptMod>,
    pub tick_acc: f32,
    rng: Rng,
}

impl ScriptHost {
    /// Compile every `.rhai` file of the given mods (alphabetical order within a mod).
    pub fn new(sources: &[ModSource]) -> (ScriptHost, Vec<String>) {
        let engine = make_engine();
        let mut mods = Vec::new();
        let mut problems = Vec::new();
        for src in sources {
            let mut ast: Option<AST> = None;
            for (name, bytes) in src.files.iter().filter(|(n, _)| n.to_ascii_lowercase().ends_with(".rhai")) {
                match engine.compile(String::from_utf8_lossy(bytes).as_ref()) {
                    Ok(a) => ast = Some(match ast {
                        Some(prev) => prev.merge(&a),
                        None => a,
                    }),
                    Err(e) => problems.push(format!("[{}] {name}: {e}", src.id)),
                }
            }
            if let Some(ast) = ast {
                let fns = ast.iter_functions().map(|f| (f.name.to_string(), f.params.len())).collect();
                mods.push(ScriptMod { id: src.id.clone(), ast, fns, vars: HashMap::new(), errors: 0, disabled: false });
            }
        }
        (ScriptHost { engine, mods, tick_acc: 0.0, rng: Rng::new(0x5C219) }, problems)
    }

    pub fn is_empty(&self) -> bool {
        self.mods.is_empty()
    }

    pub fn mod_ids(&self) -> Vec<String> {
        self.mods.iter().map(|m| m.id.clone()).collect()
    }

    /// Run `hook` in every mod that defines it with a matching number of parameters.
    pub fn call(&mut self, game: &Game, hook: &str, args: Vec<Dynamic>) -> HookResult {
        let mut out = HookResult { allow: true, ..Default::default() };
        for m in self.mods.iter_mut() {
            if m.disabled || m.fns.get(hook) != Some(&args.len()) {
                continue;
            }
            out.handled = true;
            let seed = self.rng.next_u64();
            CTX.with(|c| {
                *c.borrow_mut() = Some(Ctx { game, cmds: Vec::new(), vars: std::mem::take(&mut m.vars), log: Vec::new(), rng: Rng::new(seed) });
            });
            let result = self.engine.call_fn::<Dynamic>(&mut Scope::new(), &m.ast, hook, args.clone());
            let ctx = CTX.with(|c| c.borrow_mut().take()).expect("script context vanished");
            m.vars = ctx.vars;
            out.log.extend(ctx.log.into_iter().map(|l| format!("[{}] {l}", m.id)));
            match result {
                Ok(v) => {
                    if v.as_bool() == Ok(false) {
                        out.allow = false;
                    }
                    out.cmds.extend(ctx.cmds);
                }
                Err(e) => {
                    // A failed event's changes are thrown away, so worlds don't end up half-edited.
                    m.errors += 1;
                    let mut msg = format!("[{}] {hook}: {}", m.id, e.to_string().lines().next().unwrap_or(""));
                    if m.errors >= MAX_ERRORS {
                        m.disabled = true;
                        msg.push_str(&format!(" (scripts in {} switched off after {MAX_ERRORS} errors)", m.id));
                    }
                    out.errors.push(msg);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn src(id: &str, code: &str) -> ModSource {
        let mut files = BTreeMap::new();
        files.insert("mod.txt".to_string(), b"[mod]\n".to_vec());
        files.insert("main.rhai".to_string(), code.as_bytes().to_vec());
        ModSource { id: id.into(), files }
    }

    #[test]
    fn scripts_queue_commands_and_can_cancel() {
        let (mut host, problems) = ScriptHost::new(&[src(
            "t",
            r#"
            fn on_chat(player, text) {
                if text == "/boom" { explode(1, 2, 3, 2.5); give(player, "diamond", 3); return false; }
                if text == "/count" { let n = get_var("n"); if n == () { n = 0; } set_var("n", n + 1); message(player, `n=${n + 1}`); return false; }
                true
            }
            fn on_block_break(player, x, y, z, block) { block != "bedrock" }
            "#,
        )]);
        assert!(problems.is_empty(), "{problems:?}");
        let g = Game::new(3, false, false);
        let r = host.call(&g, "on_chat", vec!["Stove".into(), "/boom".into()]);
        assert!(r.handled && !r.allow && r.errors.is_empty(), "{:?}", r.errors);
        assert_eq!(r.cmds, vec![Cmd::Explode(Vec3::new(1.0, 2.0, 3.0), 2.5), Cmd::Give("Stove".into(), DIAMOND, 3)]);
        assert!(host.call(&g, "on_chat", vec!["Stove".into(), "hello".into()]).allow);
        // Per-mod variables persist between calls.
        host.call(&g, "on_chat", vec!["Stove".into(), "/count".into()]);
        let r = host.call(&g, "on_chat", vec!["Stove".into(), "/count".into()]);
        assert_eq!(r.cmds, vec![Cmd::Message("Stove".into(), "n=2".into())]);
        // Events a mod doesn't define are skipped.
        assert!(!host.call(&g, "on_tick", vec![Dynamic::from(0.05 as FLOAT)]).handled);
        assert!(!host.call(&g, "on_block_break", vec!["S".into(), 0.into(), 0.into(), 0.into(), "bedrock".into()]).allow);
    }

    #[test]
    fn scripts_are_sandboxed_and_bounded() {
        let (mut host, problems) = ScriptHost::new(&[src(
            "evil",
            r#"
            fn on_tick(dt) { loop { } }
            fn on_load() { fill(0, 0, 0, 1000, 100, 1000, "tnt") }
            fn on_player_join(p) { let s = "x"; loop { s += s; } }
            "#,
        )]);
        assert!(problems.is_empty(), "{problems:?}");
        let g = Game::new(3, false, false);
        let t = std::time::Instant::now();
        let r = host.call(&g, "on_tick", vec![Dynamic::from(0.05 as FLOAT)]);
        assert!(t.elapsed().as_secs() < 5, "infinite loop wasn't stopped");
        assert_eq!(r.errors.len(), 1, "{:?}", r.errors);
        // eval is rejected before the script ever runs.
        let (_, problems) = ScriptHost::new(&[src("ev", "fn on_chat(p, t) { eval(\"1 + 1\") }")]);
        assert!(problems.len() == 1 && problems[0].contains("eval"), "{problems:?}");
        let r = host.call(&g, "on_load", vec![]);
        assert!(r.cmds.is_empty() && r.errors[0].contains("too big"), "{:?}", r.errors);
        assert!(!host.call(&g, "on_player_join", vec!["a".into()]).errors.is_empty(), "string growth must be capped");
        // Imports can't read files.
        let (_, problems) = ScriptHost::new(&[src("imp", "import \"/etc/passwd\" as x; fn on_load() {}")]);
        let (mut h2, _) = ScriptHost::new(&[src("imp2", "fn on_load() { import \"secrets\" as s; }")]);
        assert!(problems.is_empty() || problems[0].contains("imp"));
        assert!(!h2.call(&g, "on_load", vec![]).errors.is_empty());
        // Too many errors switches a mod off.
        for _ in 0..MAX_ERRORS {
            host.call(&g, "on_tick", vec![Dynamic::from(0.05 as FLOAT)]);
        }
        assert!(!host.call(&g, "on_tick", vec![Dynamic::from(0.05 as FLOAT)]).handled);
    }

    #[test]
    fn the_example_script_mod_compiles_and_answers_help() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example-mods");
        let sources: Vec<ModSource> = crate::mods::read_disk(&dir).into_iter().filter(|m| m.id == "commands").collect();
        assert_eq!(sources.len(), 1);
        let (mut host, problems) = ScriptHost::new(&sources);
        assert!(problems.is_empty(), "{problems:?}");
        let g = Game::new(5, false, false);
        let r = host.call(&g, "on_chat", vec!["Stove".into(), "/help".into()]);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(!r.allow);
        assert_eq!(r.cmds.len(), 4);
        let r = host.call(&g, "on_chat", vec!["Stove".into(), "/kit".into()]);
        assert!(r.cmds.contains(&Cmd::Give("Stove".into(), crate::block::PICK_STONE, 1)), "{:?}", r.cmds);
        let r = host.call(&g, "on_chat", vec!["Stove".into(), "/kit".into()]);
        assert_eq!(r.cmds, vec![Cmd::Message("Stove".into(), "You already took your starter kit.".into())]);
        let r = host.call(&g, "on_chat", vec!["Stove".into(), "/nonsense".into()]);
        assert!(r.allow && r.errors.is_empty());
        let r = host.call(&g, "on_chat", vec!["Stove".into(), "/sethome".into()]);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let r = host.call(&g, "on_chat", vec!["Stove".into(), "/home".into()]);
        assert!(r.cmds.iter().any(|c| matches!(c, Cmd::Teleport(..))), "{:?}", r.cmds);
        let r = host.call(&g, "on_tick", vec![Dynamic::from(0.05 as FLOAT)]);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        for _ in 0..3 {
            let r = host.call(&g, "on_mob_death", vec!["oinker".into(), Dynamic::from(0.0 as FLOAT), Dynamic::from(0.0 as FLOAT), Dynamic::from(0.0 as FLOAT), "Stove".into()]);
            assert!(r.errors.is_empty(), "{:?}", r.errors);
        }
    }

    #[test]
    fn compile_errors_are_reported() {
        let (host, problems) = ScriptHost::new(&[src("broken", "fn on_load( { }")]);
        assert!(host.is_empty());
        assert_eq!(problems.len(), 1);
        assert!(problems[0].starts_with("[broken] main.rhai"));
        assert!(check("fn ok() { 1 }").is_ok());
    }
}
