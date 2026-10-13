use std::sync::LazyLock;

use crisp_core::RandomSource;

use crate::util::random::WeightedList;
use crate::world::item::dye_color::DyeColor;

enum SheepColorProvider {
    Single(DyeColor),
    Weighted(WeightedList<SheepColorProvider>),
}

impl SheepColorProvider {
    fn get(&self, random: &mut impl RandomSource) -> DyeColor {
        match self {
            SheepColorProvider::Single(color) => *color,
            SheepColorProvider::Weighted(elements) => elements.get_random_or_throw(random).get(random),
        }
    }
}

fn color(name: &str) -> DyeColor {
    DyeColor::by_name(name).expect("a dye color")
}

fn single(name: &str) -> SheepColorProvider {
    SheepColorProvider::Single(color(name))
}

fn common_colors(default_color: &str) -> SheepColorProvider {
    SheepColorProvider::Weighted(WeightedList::new(vec![(single(default_color), 499), (single("pink"), 1)]))
}

fn configuration(colors: [(&str, i32); 4], common: &str) -> SheepColorProvider {
    let mut elements: Vec<(SheepColorProvider, i32)> = colors.into_iter().map(|(name, weight)| (single(name), weight)).collect();
    elements.push((common_colors(common), 82));
    SheepColorProvider::Weighted(WeightedList::new(elements))
}

static TEMPERATE_SPAWN_CONFIGURATION: LazyLock<SheepColorProvider> = LazyLock::new(|| configuration([("black", 5), ("gray", 5), ("light_gray", 5), ("brown", 3)], "white"));
static WARM_SPAWN_CONFIGURATION: LazyLock<SheepColorProvider> = LazyLock::new(|| configuration([("gray", 5), ("light_gray", 5), ("white", 5), ("black", 3)], "brown"));
static COLD_SPAWN_CONFIGURATION: LazyLock<SheepColorProvider> = LazyLock::new(|| configuration([("light_gray", 5), ("gray", 5), ("white", 5), ("brown", 3)], "black"));

pub fn get_sheep_color(biome: i32, random: &mut impl RandomSource) -> DyeColor {
    get_sheep_color_configuration(biome).get(random)
}

fn get_sheep_color_configuration(biome: i32) -> &'static SheepColorProvider {
    if crisp_data::tag("minecraft:worldgen/biome", "minecraft:spawns_warm_variant_farm_animals").contains(&biome) {
        &WARM_SPAWN_CONFIGURATION
    } else if crisp_data::tag("minecraft:worldgen/biome", "minecraft:spawns_cold_variant_farm_animals").contains(&biome) {
        &COLD_SPAWN_CONFIGURATION
    } else {
        &TEMPERATE_SPAWN_CONFIGURATION
    }
}
