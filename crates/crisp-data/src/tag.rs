pub struct Tag {
    pub name: &'static str,
    bits: &'static [u64],
}

impl Tag {
    pub const fn new(name: &'static str, bits: &'static [u64]) -> Tag {
        Tag { name, bits }
    }

    pub fn contains(&self, id: usize) -> bool {
        self.bits.get(id / 64).is_some_and(|w| w & (1 << (id % 64)) != 0)
    }
}
