use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};

#[derive(Debug)]
pub enum CollectionContentsPredicate<P> {
    Zero,
    Single(P),
    Multiple(Vec<P>),
}

impl<P> CollectionContentsPredicate<P> {
    pub fn decode(tag: &Nbt, element: fn(&Nbt) -> DataResult<P>) -> DataResult<CollectionContentsPredicate<P>> {
        let mut predicates = codec::list_of(tag, element)?;
        Ok(match predicates.len() {
            0 => CollectionContentsPredicate::Zero,
            1 => CollectionContentsPredicate::Single(predicates.remove(0)),
            _ => CollectionContentsPredicate::Multiple(predicates),
        })
    }

    pub fn test<T>(&self, values: &[T], test: &impl Fn(&P, &T) -> bool) -> bool {
        match self {
            CollectionContentsPredicate::Zero => true,
            CollectionContentsPredicate::Single(predicate) => values.iter().any(|value| test(predicate, value)),
            CollectionContentsPredicate::Multiple(tests) => {
                let mut tests_to_match: Vec<&P> = tests.iter().collect();
                for value in values {
                    tests_to_match.retain(|p| !test(p, value));
                    if tests_to_match.is_empty() {
                        return true;
                    }
                }
                false
            }
        }
    }
}
