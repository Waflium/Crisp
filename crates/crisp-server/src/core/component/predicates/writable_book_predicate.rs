use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::collection_predicate::CollectionPredicate;
use crate::util::codec::{self, DataResult};

#[derive(Debug)]
pub struct PagePredicate {
    contents: String,
}

impl PagePredicate {
    fn decode(tag: &Nbt) -> DataResult<PagePredicate> {
        Ok(PagePredicate { contents: codec::string(tag)?.to_string() })
    }

    fn test(&self, value: &Nbt) -> bool {
        let raw = match value {
            Nbt::String(raw) => Some(raw.as_str()),
            other => other.get("raw").and_then(Nbt::as_str),
        };
        raw == Some(self.contents.as_str())
    }
}

#[derive(Debug)]
pub struct WritableBookPredicate {
    pages: Option<CollectionPredicate<PagePredicate>>,
}

impl WritableBookPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<WritableBookPredicate> {
        codec::map(tag)?;
        Ok(WritableBookPredicate { pages: codec::decode_optional(tag, "pages", |t| CollectionPredicate::decode(t, PagePredicate::decode))? })
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        let Some(pages) = &self.pages else { return true };
        pages.test(value.get("pages").map_or(&[][..], Nbt::as_list), |p, page| p.test(page))
    }
}
