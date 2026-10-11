use crisp_protocol::nbt::Nbt;

use super::collection_contents_predicate::CollectionContentsPredicate;
use super::collection_counts_predicate::CollectionCountsPredicate;
use super::min_max_bounds::Ints;
use crate::util::codec::{self, DataResult};

#[derive(Debug)]
pub struct CollectionPredicate<P> {
    contains: Option<CollectionContentsPredicate<P>>,
    counts: Option<CollectionCountsPredicate<P>>,
    size: Option<Ints>,
}

impl<P> CollectionPredicate<P> {
    pub fn decode(tag: &Nbt, element: fn(&Nbt) -> DataResult<P>) -> DataResult<CollectionPredicate<P>> {
        codec::map(tag)?;
        Ok(CollectionPredicate {
            contains: codec::decode_optional(tag, "contains", |t| CollectionContentsPredicate::decode(t, element))?,
            counts: codec::decode_optional(tag, "count", |t| CollectionCountsPredicate::decode(t, element))?,
            size: codec::decode_optional(tag, "size", Ints::decode)?,
        })
    }

    pub fn test<T>(&self, values: &[T], test: impl Fn(&P, &T) -> bool) -> bool {
        if self.contains.as_ref().is_some_and(|contains| !contains.test(values, &test)) {
            return false;
        }
        if self.counts.as_ref().is_some_and(|counts| !counts.test(values, &test)) {
            return false;
        }
        self.size.is_none_or(|size| size.matches(values.len() as i32))
    }
}
