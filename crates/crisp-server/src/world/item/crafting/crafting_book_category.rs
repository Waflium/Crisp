#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CraftingBookCategory {
    Building,
    Redstone,
    Equipment,
    Misc,
}

impl CraftingBookCategory {
    pub const VALUES: [(&'static str, CraftingBookCategory); 4] = [
        ("building", CraftingBookCategory::Building),
        ("redstone", CraftingBookCategory::Redstone),
        ("equipment", CraftingBookCategory::Equipment),
        ("misc", CraftingBookCategory::Misc),
    ];

    pub fn id(self) -> i32 {
        self as i32
    }
}
