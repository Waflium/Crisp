pub mod attribute;
pub mod clock;
pub mod container;
pub mod containers;
pub mod damagesource;
pub mod difficulty;
pub mod difficulty_instance;
pub mod effect;
pub mod entity;
pub mod flag;
pub mod food;
pub mod inventory;
pub mod item;
pub mod level;
pub mod phys;
pub mod random_sequences;
pub mod scores;
pub mod simple_container;
pub mod stopwatches;
pub mod timeline;
pub mod tick_rate_manager;
pub mod ticks;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InteractionHand {
    MainHand,
    OffHand,
}

impl InteractionHand {
    pub fn from_id(id: i32) -> InteractionHand {
        if id == 1 { InteractionHand::OffHand } else { InteractionHand::MainHand }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SwingSource {
    None,
    Predicted,
    ServerOnly,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InteractionResult {
    Success { swing_source: SwingSource, was_item_interaction: bool },
    Fail,
    Pass,
    TryWithEmptyHand,
}

impl InteractionResult {
    pub const SUCCESS: InteractionResult = InteractionResult::Success { swing_source: SwingSource::Predicted, was_item_interaction: true };
    pub const SUCCESS_SERVER: InteractionResult = InteractionResult::Success { swing_source: SwingSource::ServerOnly, was_item_interaction: true };
    pub const CONSUME: InteractionResult = InteractionResult::Success { swing_source: SwingSource::None, was_item_interaction: true };

    pub fn consumes_action(self) -> bool {
        matches!(self, InteractionResult::Success { .. })
    }

    pub fn should_swing(self) -> bool {
        matches!(self, InteractionResult::Success { swing_source, .. } if swing_source != SwingSource::None)
    }
}
