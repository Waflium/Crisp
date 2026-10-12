#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DyeColor {
    pub id: i32,
    pub name: &'static str,
    texture_diffuse_color: i32,
    firework_color: i32,
}

const fn dye(id: i32, name: &'static str, texture_diffuse_color: i32, firework_color: i32) -> DyeColor {
    DyeColor { id, name, texture_diffuse_color: texture_diffuse_color | 0xFF00_0000u32 as i32, firework_color }
}

impl DyeColor {
    pub const VALUES: [DyeColor; 16] = [
        dye(0, "white", 16383998, 15790320),
        dye(1, "orange", 16351261, 15435844),
        dye(2, "magenta", 13061821, 12801229),
        dye(3, "light_blue", 3847130, 6719955),
        dye(4, "yellow", 16701501, 14602026),
        dye(5, "lime", 8439583, 4312372),
        dye(6, "pink", 15961002, 14188952),
        dye(7, "gray", 4673362, 4408131),
        dye(8, "light_gray", 10329495, 11250603),
        dye(9, "cyan", 1481884, 2651799),
        dye(10, "purple", 8991416, 8073150),
        dye(11, "blue", 3949738, 2437522),
        dye(12, "brown", 8606770, 5320730),
        dye(13, "green", 6192150, 3887386),
        dye(14, "red", 11546150, 11743532),
        dye(15, "black", 1908001, 1973019),
    ];

    pub fn texture_diffuse_color(self) -> i32 {
        self.texture_diffuse_color
    }

    pub fn get_firework_color(self) -> i32 {
        self.firework_color
    }

    pub fn by_name(name: &str) -> Option<DyeColor> {
        DyeColor::VALUES.iter().find(|color| color.name == name).copied()
    }
}
