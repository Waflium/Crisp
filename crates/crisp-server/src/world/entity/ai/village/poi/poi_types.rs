use std::sync::LazyLock;

use crisp_data::properties::{BED_PART, BedPart};
use crisp_data::{BlockState, blocks};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PoiType {
    Armorer,
    Butcher,
    Cartographer,
    Cleric,
    Farmer,
    Fisherman,
    Fletcher,
    Leatherworker,
    Librarian,
    Mason,
    Shepherd,
    Toolsmith,
    Weaponsmith,
    Home,
    Meeting,
    Beehive,
    BeeNest,
    NetherPortal,
    Lodestone,
    TestInstance,
    LightningRod,
}

impl PoiType {
    pub const VALUES: [PoiType; 21] = [
        PoiType::Armorer,
        PoiType::Butcher,
        PoiType::Cartographer,
        PoiType::Cleric,
        PoiType::Farmer,
        PoiType::Fisherman,
        PoiType::Fletcher,
        PoiType::Leatherworker,
        PoiType::Librarian,
        PoiType::Mason,
        PoiType::Shepherd,
        PoiType::Toolsmith,
        PoiType::Weaponsmith,
        PoiType::Home,
        PoiType::Meeting,
        PoiType::Beehive,
        PoiType::BeeNest,
        PoiType::NetherPortal,
        PoiType::Lodestone,
        PoiType::TestInstance,
        PoiType::LightningRod,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PoiType::Armorer => "minecraft:armorer",
            PoiType::Butcher => "minecraft:butcher",
            PoiType::Cartographer => "minecraft:cartographer",
            PoiType::Cleric => "minecraft:cleric",
            PoiType::Farmer => "minecraft:farmer",
            PoiType::Fisherman => "minecraft:fisherman",
            PoiType::Fletcher => "minecraft:fletcher",
            PoiType::Leatherworker => "minecraft:leatherworker",
            PoiType::Librarian => "minecraft:librarian",
            PoiType::Mason => "minecraft:mason",
            PoiType::Shepherd => "minecraft:shepherd",
            PoiType::Toolsmith => "minecraft:toolsmith",
            PoiType::Weaponsmith => "minecraft:weaponsmith",
            PoiType::Home => "minecraft:home",
            PoiType::Meeting => "minecraft:meeting",
            PoiType::Beehive => "minecraft:beehive",
            PoiType::BeeNest => "minecraft:bee_nest",
            PoiType::NetherPortal => "minecraft:nether_portal",
            PoiType::Lodestone => "minecraft:lodestone",
            PoiType::TestInstance => "minecraft:test_instance",
            PoiType::LightningRod => "minecraft:lightning_rod",
        }
    }

    pub fn by_name(name: &str) -> Option<PoiType> {
        PoiType::VALUES.into_iter().find(|t| t.name() == name)
    }

    pub fn max_tickets(self) -> i32 {
        match self {
            PoiType::Meeting => 32,
            PoiType::Beehive | PoiType::BeeNest | PoiType::NetherPortal | PoiType::Lodestone | PoiType::TestInstance | PoiType::LightningRod => 0,
            _ => 1,
        }
    }

    pub fn valid_range(self) -> i32 {
        match self {
            PoiType::Meeting => 6,
            _ => 1,
        }
    }
}

static TYPE_BY_STATE: LazyLock<Vec<Option<PoiType>>> =
    LazyLock::new(|| (0..BlockState::count() as u32).map(|id| matching_type(BlockState::by_id(id).expect("a block state"))).collect());

pub fn for_state(state: BlockState) -> Option<PoiType> {
    TYPE_BY_STATE[state.id() as usize]
}

fn matching_type(state: BlockState) -> Option<PoiType> {
    let block = state.block();
    Some(match block {
        _ if block == blocks::BLAST_FURNACE => PoiType::Armorer,
        _ if block == blocks::SMOKER => PoiType::Butcher,
        _ if block == blocks::CARTOGRAPHY_TABLE => PoiType::Cartographer,
        _ if block == blocks::BREWING_STAND => PoiType::Cleric,
        _ if block == blocks::COMPOSTER => PoiType::Farmer,
        _ if block == blocks::BARREL => PoiType::Fisherman,
        _ if block == blocks::FLETCHING_TABLE => PoiType::Fletcher,
        _ if block == blocks::CAULDRON || block == blocks::LAVA_CAULDRON || block == blocks::WATER_CAULDRON || block == blocks::POWDER_SNOW_CAULDRON => {
            PoiType::Leatherworker
        }
        _ if block == blocks::LECTERN => PoiType::Librarian,
        _ if block == blocks::STONECUTTER => PoiType::Mason,
        _ if block == blocks::LOOM => PoiType::Shepherd,
        _ if block == blocks::SMITHING_TABLE => PoiType::Toolsmith,
        _ if block == blocks::GRINDSTONE => PoiType::Weaponsmith,
        _ if block.is_instance_of("BedBlock") && state.get(BED_PART) == BedPart::Head => PoiType::Home,
        _ if block == blocks::BELL => PoiType::Meeting,
        _ if block == blocks::BEEHIVE => PoiType::Beehive,
        _ if block == blocks::BEE_NEST => PoiType::BeeNest,
        _ if block == blocks::NETHER_PORTAL => PoiType::NetherPortal,
        _ if block == blocks::LODESTONE => PoiType::Lodestone,
        _ if block == blocks::TEST_INSTANCE_BLOCK => PoiType::TestInstance,
        _ if block.is_instance_of("LightningRodBlock") => PoiType::LightningRod,
        _ => return None,
    })
}

pub fn has_poi(state: BlockState) -> bool {
    for_state(state).is_some()
}
