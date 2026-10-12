use crisp_core::BlockPos;
use crisp_protocol::nbt::Nbt;

pub const SPAWN_TIME: i64 = 200;
pub const COOLDOWN_TIME: i32 = 40;
pub const ATTENTION_INTERVAL: i64 = 2400;
const DEFAULT_AGE: i64 = 0;
const DEFAULT_EXACT_TELEPORT: bool = false;

#[derive(Clone, Default)]
pub struct TheEndGatewayBlockEntity {
    pub age: i64,
    pub teleport_cooldown: i32,
    pub exit_portal: Option<BlockPos>,
    pub exact_teleport: bool,
}

impl TheEndGatewayBlockEntity {
    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("Age".into(), Nbt::Long(self.age)));
        if let Some(exit_portal) = self.exit_portal {
            output.push(("exit_portal".into(), Nbt::IntArray(vec![exit_portal.x, exit_portal.y, exit_portal.z])));
        }
        if self.exact_teleport {
            output.push(("ExactTeleport".into(), Nbt::Byte(1)));
        }
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.age = input.get("Age").and_then(super::long_value).unwrap_or(DEFAULT_AGE);
        self.exit_portal = match input.get("exit_portal") {
            Some(Nbt::IntArray(v)) if v.len() == 3 => Some(BlockPos::new(v[0], v[1], v[2])).filter(|&pos| is_in_spawnable_bounds(pos)),
            _ => None,
        };
        self.exact_teleport = super::int_or(input, "ExactTeleport", DEFAULT_EXACT_TELEPORT as i32) != 0;
    }

    pub fn is_spawning(&self) -> bool {
        self.age < SPAWN_TIME
    }

    pub fn is_cooling_down(&self) -> bool {
        self.teleport_cooldown > 0
    }

    pub fn set_exit_position(&mut self, exit_portal: BlockPos, exact_teleport: bool) {
        self.exact_teleport = exact_teleport;
        self.exit_portal = Some(exit_portal);
    }
}

fn is_in_spawnable_bounds(pos: BlockPos) -> bool {
    (-20_000_000..20_000_000).contains(&pos.y) && (-30_000_000..30_000_000).contains(&pos.x) && (-30_000_000..30_000_000).contains(&pos.z)
}
