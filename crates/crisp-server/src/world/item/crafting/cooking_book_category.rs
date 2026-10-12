#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CookingBookCategory {
    Food,
    Blocks,
    Misc,
}

impl CookingBookCategory {
    pub const VALUES: [(&'static str, CookingBookCategory); 3] = [("food", CookingBookCategory::Food), ("blocks", CookingBookCategory::Blocks), ("misc", CookingBookCategory::Misc)];

    pub fn id(self) -> i32 {
        self as i32
    }
}
