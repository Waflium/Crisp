use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Abilities {
    pub invulnerable: bool,
    pub flying: bool,
    pub mayfly: bool,
    pub instabuild: bool,
    pub may_build: bool,
    pub flying_speed: f32,
    pub walking_speed: f32,
}

impl Default for Abilities {
    fn default() -> Abilities {
        Abilities { invulnerable: false, flying: false, mayfly: false, instabuild: false, may_build: true, flying_speed: 0.05, walking_speed: 0.1 }
    }
}

impl Abilities {
    pub fn write(&self, w: &mut Writer) {
        let flags = self.invulnerable as u8 | (self.flying as u8) << 1 | (self.mayfly as u8) << 2 | (self.instabuild as u8) << 3;
        w.u8(flags);
        w.f32(self.flying_speed);
        w.f32(self.walking_speed);
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(vec![
            ("invulnerable".into(), Nbt::Byte(self.invulnerable as i8)),
            ("flying".into(), Nbt::Byte(self.flying as i8)),
            ("mayfly".into(), Nbt::Byte(self.mayfly as i8)),
            ("instabuild".into(), Nbt::Byte(self.instabuild as i8)),
            ("mayBuild".into(), Nbt::Byte(self.may_build as i8)),
            ("flySpeed".into(), Nbt::Float(self.flying_speed)),
            ("walkSpeed".into(), Nbt::Float(self.walking_speed)),
        ])
    }

    pub fn load(nbt: &Nbt) -> Abilities {
        let d = Abilities::default();
        let flag = |key: &str, default: bool| nbt.get(key).and_then(Nbt::as_i64).map_or(default, |v| v != 0);
        let float = |key: &str, default: f32| nbt.get(key).and_then(Nbt::as_f64).map_or(default, |v| v as f32);
        Abilities {
            invulnerable: flag("invulnerable", d.invulnerable),
            flying: flag("flying", d.flying),
            mayfly: flag("mayfly", d.mayfly),
            instabuild: flag("instabuild", d.instabuild),
            may_build: flag("mayBuild", d.may_build),
            flying_speed: float("flySpeed", d.flying_speed),
            walking_speed: float("walkSpeed", d.walking_speed),
        }
    }
}
