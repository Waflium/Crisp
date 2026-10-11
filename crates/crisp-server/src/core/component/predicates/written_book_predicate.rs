use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::collection_predicate::CollectionPredicate;
use crate::advancements::predicates::min_max_bounds::Ints;
use crate::network::chat::Component;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec as component_codec;

fn raw(value: &Nbt) -> &Nbt {
    match value {
        Nbt::Compound(_) => value.get("raw").unwrap_or(value),
        other => other,
    }
}

#[derive(Debug)]
pub struct PagePredicate {
    contents: Component,
}

impl PagePredicate {
    fn decode(tag: &Nbt) -> DataResult<PagePredicate> {
        Ok(PagePredicate { contents: crate::network::chat::component_serialization::decode(tag)? })
    }

    fn test(&self, value: &Nbt) -> bool {
        crate::network::chat::component_serialization::decode(raw(value)).is_ok_and(|page| page == self.contents)
    }
}

#[derive(Debug)]
pub struct WrittenBookPredicate {
    pages: Option<CollectionPredicate<PagePredicate>>,
    author: Option<String>,
    title: Option<String>,
    generation: Ints,
    resolved: Option<bool>,
}

impl WrittenBookPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<WrittenBookPredicate> {
        codec::map(tag)?;
        Ok(WrittenBookPredicate {
            pages: codec::decode_optional(tag, "pages", |t| CollectionPredicate::decode(t, PagePredicate::decode))?,
            author: codec::decode_optional(tag, "author", |t| Ok(codec::string(t)?.to_string()))?,
            title: codec::decode_optional(tag, "title", |t| Ok(codec::string(t)?.to_string()))?,
            generation: codec::decode_or(tag, "generation", Ints::ANY, Ints::decode)?,
            resolved: codec::decode_optional(tag, "resolved", codec::bool_value)?,
        })
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        if self.author.as_ref().is_some_and(|author| value.get("author").and_then(Nbt::as_str) != Some(author.as_str())) {
            return false;
        }
        if self.title.as_ref().is_some_and(|title| value.get("title").map(raw).and_then(Nbt::as_str) != Some(title.as_str())) {
            return false;
        }
        if !self.generation.matches(value.get("generation").map_or(0, component_codec::as_i32)) {
            return false;
        }
        if self.resolved.is_some_and(|resolved| resolved != value.get("resolved").is_some_and(|v| component_codec::as_i64(v) != 0)) {
            return false;
        }
        self.pages.as_ref().is_none_or(|pages| pages.test(value.get("pages").map_or(&[][..], Nbt::as_list), |p, page| p.test(page)))
    }
}
