//! Native controls for the command-driven example machines. Scripts still own every recipe.
use crate::{block::*, game::Game, net::Msg};
use macroquad::math::IVec3;

pub struct Action {
    pub label: &'static str,
    pub command: &'static str,
    pub inputs: &'static [(&'static str, u32)],
}
pub struct Spec {
    pub hint: &'static str,
    pub actions: &'static [Action],
}
const REFINE: &[Action] = &[Action { label: "Refine pellet", command: "/hbm refine", inputs: &[("half_baked:glowium_dust", 2), ("coal", 1)] }];
const REACTOR: &[Action] = &[
    Action { label: "Start 15-second batch", command: "/hbm reactor", inputs: &[("half_baked:pellet", 1), ("half_baked:coolant", 1)] },
    Action { label: "Scram owned batches", command: "/hbm scram", inputs: &[] },
];
const FUEL: &[Action] = &[Action { label: "Refine 2 fuel canisters", command: "/astra refine", inputs: &[("coal", 2), ("ad_absurdum:tank", 1)] }];
const OXYGEN: &[Action] =
    &[Action { label: "Fill 4 oxygen canisters", command: "/astra refill", inputs: &[("ad_absurdum:battery", 1), ("ad_absurdum:tank", 1)] }];
const SOLAR: &[Action] = &[Action { label: "Charge 2 batteries", command: "/astra charge", inputs: &[("ad_absurdum:steel", 1), ("zap_dust", 1)] }];
const ROCKET: &[Action] = &[
    Action { label: "Assemble tier 1", command: "/astra assemble 1", inputs: &[("ad_absurdum:rocket_1", 1)] },
    Action { label: "Assemble tier 2", command: "/astra assemble 2", inputs: &[("ad_absurdum:rocket_2", 1)] },
    Action { label: "Assemble tier 3", command: "/astra assemble 3", inputs: &[("ad_absurdum:rocket_3", 1)] },
    Action { label: "Assemble tier 4", command: "/astra assemble 4", inputs: &[("ad_absurdum:rocket_4", 1)] },
    Action { label: "Pack owned rockets", command: "/astra pack", inputs: &[] },
];
pub fn spec(id: Id) -> Option<Spec> {
    Some(match reg().key_of(id) {
        "half_baked:centrifuge" => Spec { hint: "Turns dust and coal into one pellet.", actions: REFINE },
        "half_baked:reactor" => Spec { hint: "Keep the cooling tower in place. Output: 4 cells + waste.", actions: REACTOR },
        "half_baked:cooling_tower" => Spec { hint: "Passive cooling for reactors within three blocks.", actions: &[] },
        "ad_absurdum:refinery" => Spec { hint: "Refines coal into rocket fuel.", actions: FUEL },
        "ad_absurdum:oxygen_generator" => Spec { hint: "Fills oxygen for your Life Support Pack.", actions: OXYGEN },
        "ad_absurdum:solar_panel" => Spec { hint: "Each recharge needs 60 seconds between batches.", actions: SOLAR },
        "ad_absurdum:rocket_workbench" | "ad_absurdum:launch_pad" => {
            Spec { hint: "Bench and pad must be nearby, with five clear blocks above.", actions: ROCKET }
        }
        _ => return None,
    })
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
            self.fire("on_chat", vec![name.into(), a.command.into()]);
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
            return if let Some(v) = active {
                ("Cooling active. Steam venting.".into(), v.progress)
            } else {
                ("Cooling ready. Keep this tower beside the reactor.".into(), 0)
            };
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
                        return (format!("Batch running: {left:.0}s left. Keep cooling connected."), ((1.0 - left / 15.0) * 255.0) as u8);
                    }
                }
            }
        }
        if key == "ad_absurdum:solar_panel" {
            let left = host.variable("ad_absurdum", &format!("charge_{name}")).as_ref().and_then(number).unwrap_or(0.0);
            if left > 0.0 {
                return (format!("Recharging: {left:.0}s until the next batch."), ((1.0 - left / 60.0).clamp(0.0, 1.0) * 255.0) as u8);
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
