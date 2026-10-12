use crisp_core::BlockPos;
use crisp_data::Item;

use super::ItemStack;
use crate::server::level::ServerLevel;
use crate::world::level::block::entity::sign_block_entity::SignTextSlot;
use crate::world::level::block::entity::sign_text::SignText;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SignApplicator {
    Dye,
    GlowInkSac,
    InkSac,
    Honeycomb,
}

impl SignApplicator {
    pub fn of(item: Item) -> Option<SignApplicator> {
        Some(match item.info().classes.first() {
            Some(&"DyeItem") => SignApplicator::Dye,
            Some(&"GlowInkSacItem") => SignApplicator::GlowInkSac,
            Some(&"InkSacItem") => SignApplicator::InkSac,
            Some(&"HoneycombItem") => SignApplicator::Honeycomb,
            _ => return None,
        })
    }

    pub fn try_apply_to_sign(self, level: &mut ServerLevel, pos: BlockPos, slot: SignTextSlot, item: &ItemStack) -> bool {
        match self {
            SignApplicator::Dye => super::dye_item::try_apply_to_sign(level, pos, slot, item),
            SignApplicator::GlowInkSac => super::glow_ink_sac_item::try_apply_to_sign(level, pos, slot),
            SignApplicator::InkSac => super::ink_sac_item::try_apply_to_sign(level, pos, slot),
            SignApplicator::Honeycomb => super::honeycomb_item::try_apply_to_sign(level, pos),
        }
    }

    pub fn can_apply_to_sign(self, text: &SignText, text_filtering_enabled: bool) -> bool {
        match self {
            SignApplicator::Honeycomb => true,
            _ => text.has_message(text_filtering_enabled),
        }
    }
}
