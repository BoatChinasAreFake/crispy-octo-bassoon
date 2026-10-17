//! Native controls for the command-driven example machines. Scripts still own every recipe.
use crate::{block::*, game::Game, net::Msg};
use macroquad::math::IVec3;

pub use crate::block::MachineAction as Action;
pub struct Spec {
    pub hint: &'static str,
    pub actions: &'static [Action],
}
/// A command-driven mod machine's panel: its text and buttons come from the mod (`[machine ...]`).
pub fn spec(id: Id) -> Option<Spec> {
    let m = reg().machine(id)?;
    (crate::factory::kind(id).is_none()).then_some(Spec { hint: &m.hint, actions: &m.actions })
}
#[derive(Clone)]
pub struct Panel {
    pub dim: crate::dims::Dim,
    pub pos: IVec3,
    pub block: Id,
    pub status: String,
    pub progress: u8,
    pub refresh: f32,
}
impl Game {
    pub fn open_machine(&mut self, pos: IVec3) -> bool {
        let id = self.world.get_v(pos);
        if crate::factory::kind(id).is_some() {
            self.open_container(pos);
            self.advance("push_buttons");
            return true;
        }
        if spec(id).is_none() {
            return false;
        }
        self.machine = Some(Panel { dim: self.realm_dim(), pos, block: id, status: "Checking machine...".into(), progress: 0, refresh: 0.0 });
        self.machine_action(255);
        self.advance("push_buttons");
        true
    }
    pub fn machine_still_there(&self) -> bool {
        self.machine.as_ref().is_some_and(|m| {
            self.dead.is_none() && !self.spectator && m.dim == self.realm_dim() && self.world.get_v(m.pos) == m.block && near(self.player.body.pos, m.pos)
        })
    }
    pub fn machine_tick(&mut self, dt: f32) {
        if !self.machine_still_there() {
            self.machine = None;
            return;
        }
        let m = self.machine.as_mut().unwrap();
        m.refresh -= dt;
        if m.refresh <= 0.0 {
            m.refresh = 1.0;
            self.machine_action(255);
        }
    }
    pub fn machine_action(&mut self, action: u8) {
        let Some(m) = self.machine.as_ref() else { return };
        let p = m.pos;
        if self.is_client() {
            self.net_send_msg(Msg::MachineUse { x: p.x, y: p.y, z: p.z, action });
        } else {
            let name = self.player_name.clone();
            self.operate_machine(self.my_id, &name, p, action);
        }
    }
    pub(crate) fn operate_machine(&mut self, who: u32, name: &str, pos: IVec3, action: u8) {
        if crate::factory::kind(self.world.get_v(pos)).is_some() {
            self.factory_control(who, pos, action);
            return;
        }
        let player = if who == self.my_id {
            (!self.spectator && self.dead.is_none()).then_some(self.player.body.pos)
        } else {
            self.peers.get(&who).filter(|p| p.alive()).map(|p| p.target)
        };
        let Some(player) = player else { return };
        if !near(player, pos) {
            return;
        }
        let id = self.world.get_v(pos);
        let Some(spec) = spec(id) else { return };
        // An index into an allowlist, never a client-provided command or recipe.
        if action != 255 {
            let Some(a) = spec.actions.get(action as usize) else { return };
            if who != self.my_id && !self.peer_rate_ok(who, "machine_action", 0.3) {
                return;
            }
            self.fire("on_chat", vec![name.into(), a.command.clone().into()]);
        } else if who != self.my_id && !self.peer_rate_ok(who, "machine_status", 0.4) {
            return;
        }
        let (status, progress) = self.machine_status(id, pos, name);
        if who == self.my_id {
            if let Some(m) = self.machine.as_mut() {
                m.status = status;
                m.progress = progress;
            }
        } else {
            self.net_send_to(who, Msg::MachineInfo { x: pos.x, y: pos.y, z: pos.z, block: id, status, progress });
        }
    }
    fn machine_status(&self, id: Id, pos: IVec3, name: &str) -> (String, u8) {
        if reg().key_of(id) == "half_baked:cooling_tower" {
            let active = self
                .world
                .containers
                .keys()
                .filter(|p| (**p - pos).abs().max_element() <= 3)
                .filter_map(|p| self.factory_visual(*p))
                .find(|v| crate::factory::kind(v.block) == Some(crate::factory::Kind::Reactor) && v.state == 2);
            let text = |name: &str| reg().machine(id).and_then(|m| m.status(name, 0.0)).unwrap_or_default();
            return if let Some(v) = active { (text("active"), v.progress) } else { (text("idle"), 0) };
        }
        let Some(host) = self.scripts.as_ref() else { return ("Machine script is not installed.".into(), 0) };
        let key = reg().key_of(id);
        let pack = key.split(':').next().unwrap_or("");
        if !host.mod_ids().iter().any(|s| s == pack) {
            return ("Machine script is not installed.".into(), 0);
        }
        if (key == "half_baked:reactor" || key == "half_baked:cooling_tower")
            && let Some(jobs) = host.variable("half_baked", "reactors").and_then(|v| v.try_cast::<rhai::Array>())
        {
            for job in jobs {
                if let Some(m) = job.try_cast::<rhai::Map>() {
                    let owner = m.get("owner").map(ToString::to_string).unwrap_or_default();
                    let dim = m.get("dim").map(ToString::to_string).unwrap_or_default();
                    let at = m.get(if key.ends_with("reactor") { "at" } else { "tower" }).and_then(|v| v.clone().try_cast::<rhai::Array>());
                    if owner == name
                        && dim == crate::scripting::dimension_key(self.realm_dim())
                        && at.is_some_and(|a| a.len() == 3 && a.iter().zip([pos.x, pos.y, pos.z]).all(|(v, p)| number(v) == Some(p as f32)))
                    {
                        let left = m.get("left").and_then(number).unwrap_or(15.0).clamp(0.0, 15.0);
                        let text = reg().machine(id).and_then(|mm| mm.status("running", left)).unwrap_or_default();
                        return (text, ((1.0 - left / 15.0) * 255.0) as u8);
                    }
                }
            }
        }
        if key == "ad_absurdum:solar_panel" {
            let left = host.variable("ad_absurdum", &format!("charge_{name}")).as_ref().and_then(number).unwrap_or(0.0);
            if left > 0.0 {
                let text = reg().machine(id).and_then(|m| m.status("running", left)).unwrap_or_default();
                return (text, ((1.0 - left / 60.0).clamp(0.0, 1.0) * 255.0) as u8);
            }
        }
        ("Ready. Ingredients come from your inventory.".into(), 0)
    }
}
fn number(v: &rhai::Dynamic) -> Option<f32> {
    v.as_int().ok().map(|n| n as f32).or_else(|| v.as_float().ok().map(|n| n as f32))
}
fn near(p: macroquad::math::Vec3, b: IVec3) -> bool {
    p.is_finite() && p.floor().as_ivec3().to_array().into_iter().zip(b.to_array()).all(|(a, b)| (a as i64 - b as i64).abs() <= 3)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn machine_reach_handles_malformed_wire_coordinates_without_overflow() {
        assert!(!near(macroquad::math::Vec3::ZERO, IVec3::splat(i32::MIN)));
        assert!(!near(macroquad::math::Vec3::ZERO, IVec3::splat(i32::MAX)));
        assert!(!near(macroquad::math::Vec3::splat(f32::NAN), IVec3::ZERO));
        assert!(near(macroquad::math::Vec3::new(1.5, 90.0, 1.5), IVec3::new(4, 90, 1)));
        assert!(!near(macroquad::math::Vec3::new(1.5, 90.0, 1.5), IVec3::new(5, 90, 1)));
    }
}
