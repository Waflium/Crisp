use super::map_decoration_type::{MapDecorationType, MapDecorationTypeHolder};

const fn register(name: &'static str, asset_id: &'static str, show_on_item_frame: bool, track_count: bool) -> MapDecorationType {
    MapDecorationType { name, asset_id, show_on_item_frame, track_count }
}

pub static TYPES: [MapDecorationType; 40] = [
    register("player", "player", false, true),
    register("frame", "frame", true, true),
    register("red_marker", "red_marker", false, true),
    register("blue_marker", "blue_marker", false, true),
    register("target_x", "target_x", true, false),
    register("target_point", "target_point", true, false),
    register("player_off_map", "player_off_map", false, true),
    register("player_off_limits", "player_off_limits", false, true),
    register("mansion", "woodland_mansion", true, false),
    register("monument", "ocean_monument", true, false),
    register("banner_white", "white_banner", true, true),
    register("banner_orange", "orange_banner", true, true),
    register("banner_magenta", "magenta_banner", true, true),
    register("banner_light_blue", "light_blue_banner", true, true),
    register("banner_yellow", "yellow_banner", true, true),
    register("banner_lime", "lime_banner", true, true),
    register("banner_pink", "pink_banner", true, true),
    register("banner_gray", "gray_banner", true, true),
    register("banner_light_gray", "light_gray_banner", true, true),
    register("banner_cyan", "cyan_banner", true, true),
    register("banner_purple", "purple_banner", true, true),
    register("banner_blue", "blue_banner", true, true),
    register("banner_brown", "brown_banner", true, true),
    register("banner_green", "green_banner", true, true),
    register("banner_red", "red_banner", true, true),
    register("banner_black", "black_banner", true, true),
    register("red_x", "red_x", true, false),
    register("village_desert", "desert_village", true, false),
    register("village_plains", "plains_village", true, false),
    register("village_savanna", "savanna_village", true, false),
    register("village_snowy", "snowy_village", true, false),
    register("village_taiga", "taiga_village", true, false),
    register("jungle_temple", "jungle_temple", true, false),
    register("swamp_hut", "swamp_hut", true, false),
    register("trial_chambers", "trial_chambers", true, false),
    register("abandoned_camp", "abandoned_camp", true, false),
    register("ancient_city", "ancient_city", true, false),
    register("desert_pyramid", "desert_pyramid", true, false),
    register("mineshaft", "mineshaft", true, false),
    register("ocean_ruin_warm", "warm_ocean_ruins", true, false),
];

pub const PLAYER: MapDecorationTypeHolder = MapDecorationTypeHolder(0);
pub const FRAME: MapDecorationTypeHolder = MapDecorationTypeHolder(1);
pub const PLAYER_OFF_MAP: MapDecorationTypeHolder = MapDecorationTypeHolder(6);
pub const PLAYER_OFF_LIMITS: MapDecorationTypeHolder = MapDecorationTypeHolder(7);
pub const WOODLAND_MANSION: MapDecorationTypeHolder = MapDecorationTypeHolder(8);
pub const WHITE_BANNER: MapDecorationTypeHolder = MapDecorationTypeHolder(10);
