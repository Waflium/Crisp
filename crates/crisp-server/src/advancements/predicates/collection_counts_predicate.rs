use crisp_protocol::nbt::Nbt;

use super::min_max_bounds::Ints;
use crate::util::codec::{self, DataResult};

#[derive(Debug)]
pub struct Entry<P> {
    test: P,
    count: Ints,
}

impl<P> Entry<P> {
    fn test<T>(&self, values: &[T], test: &impl Fn(&P, &T) -> bool) -> bool {
        let count = values.iter().filter(|value| test(&self.test, value)).count();
        self.count.matches(count as i32)
    }
}

#[derive(Debug)]
pub struct CollectionCountsPredicate<P> {
    entries: Vec<Entry<P>>,
}

impl<P> CollectionCountsPredicate<P> {
    pub fn decode(tag: &Nbt, element: fn(&Nbt) -> DataResult<P>) -> DataResult<CollectionCountsPredicate<P>> {
        let entries = codec::list_of(tag, |t| Ok(Entry { test: codec::decode_field(t, "test", element)?, count: codec::decode_field(t, "count", Ints::decode)? }))?;
        Ok(CollectionCountsPredicate { entries })
    }

    pub fn test<T>(&self, values: &[T], test: &impl Fn(&P, &T) -> bool) -> bool {
        self.entries.iter().all(|entry| entry.test(values, test))
    }
}
