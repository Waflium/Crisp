use std::collections::HashMap;

use crisp_core::BlockPos;
use crisp_core::random::{LegacyRandomSource, RandomSource};
use serde_json::Value;

use crate::util::random::WeightedList;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;

pub enum PoolAliasBinding {
    Direct { alias: String, target: String },
    Random { alias: String, targets: WeightedList<String> },
    RandomGroup { groups: WeightedList<Vec<PoolAliasBinding>> },
}

impl PoolAliasBinding {
    pub fn parse(json: &Value) -> Result<PoolAliasBinding, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("pool alias without type")?;
        let string = |key: &str| json.get(key).and_then(Value::as_str).map(resource).ok_or_else(|| format!("missing {key}"));
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "direct" => PoolAliasBinding::Direct { alias: string("alias")?, target: string("target")? },
            "random" => PoolAliasBinding::Random {
                alias: string("alias")?,
                targets: WeightedList::parse(json.get("targets").ok_or("missing targets")?, |v| v.as_str().map(resource).ok_or_else(|| "pool ids are strings".to_owned()))?,
            },
            "random_group" => PoolAliasBinding::RandomGroup {
                groups: WeightedList::parse(json.get("groups").ok_or("missing groups")?, |v| {
                    v.as_array().ok_or("a group is a list")?.iter().map(PoolAliasBinding::parse).collect::<Result<Vec<_>, _>>()
                })?,
            },
            other => return Err(format!("unknown pool alias {other}")),
        })
    }

    fn for_each_resolved(&self, random: &mut impl RandomSource, consumer: &mut impl FnMut(&str, &str)) {
        match self {
            PoolAliasBinding::Direct { alias, target } => consumer(alias, target),
            PoolAliasBinding::Random { alias, targets } => consumer(alias, targets.get_random_or_throw(random)),
            PoolAliasBinding::RandomGroup { groups } => {
                for binding in groups.get_random_or_throw(random) {
                    binding.for_each_resolved(random, consumer);
                }
            }
        }
    }
}

pub struct PoolAliasLookup {
    alias_mappings: HashMap<String, String>,
}

impl PoolAliasLookup {
    pub fn create(pool_alias_bindings: &[PoolAliasBinding], pos: BlockPos, seed: i64) -> PoolAliasLookup {
        let mut alias_mappings = HashMap::new();
        if !pool_alias_bindings.is_empty() {
            let mut random = LegacyRandomSource::new(seed).fork_positional().at(pos.x, pos.y, pos.z);
            for binding in pool_alias_bindings {
                binding.for_each_resolved(&mut random, &mut |alias, target| {
                    alias_mappings.insert(alias.to_owned(), target.to_owned());
                });
            }
        }
        PoolAliasLookup { alias_mappings }
    }

    pub fn lookup<'a>(&'a self, alias: &'a str) -> &'a str {
        self.alias_mappings.get(alias).map_or(alias, String::as_str)
    }
}
