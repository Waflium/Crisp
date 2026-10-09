use crisp_core::mth;

pub fn damage_after_absorb(damage: f32, total_armor: f32, armor_toughness: f32) -> f32 {
    let toughness = 2.0 + armor_toughness / 4.0;
    let real_armor = mth::clamp_f(total_armor - damage / toughness, total_armor * 0.2, 20.0);
    let armor_fraction = real_armor / 25.0;
    let damage_multiplier = 1.0 - armor_fraction;
    damage * damage_multiplier
}
