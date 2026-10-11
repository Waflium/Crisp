use std::collections::{BTreeMap, BTreeSet};

use crisp_core::hash_map::FastMap;

use super::biome_generation_settings;
use crate::world::level::chunk::Biome;

pub struct StepFeatureData {
    pub features: Vec<u32>,
    index_mapping: FastMap<u32, usize>,
}

impl StepFeatureData {
    fn new(features: Vec<u32>) -> StepFeatureData {
        let mut index_mapping = FastMap::default();
        for (i, &feature) in features.iter().enumerate() {
            index_mapping.entry(feature).or_insert(i);
        }
        StepFeatureData { features, index_mapping }
    }

    pub fn index_of(&self, feature: u32) -> usize {
        self.index_mapping[&feature]
    }
}

type FeatureData = (usize, usize);

pub fn build_features_per_step(feature_sources: &[Biome]) -> Vec<StepFeatureData> {
    let mut feature_index: FastMap<u32, usize> = FastMap::default();
    let mut features_by_index: Vec<u32> = Vec::new();
    let mut edges: BTreeMap<FeatureData, BTreeSet<FeatureData>> = BTreeMap::new();
    let mut max_step = 0;
    for &feature_source in feature_sources {
        let features_for_step = biome_generation_settings::get(feature_source).features();
        max_step = max_step.max(features_for_step.len());
        let mut feature_list: Vec<FeatureData> = Vec::new();
        for (i, step) in features_for_step.iter().enumerate() {
            for &feature in step {
                let index = *feature_index.entry(feature).or_insert_with(|| {
                    features_by_index.push(feature);
                    features_by_index.len() - 1
                });
                feature_list.push((i, index));
            }
        }
        for i in 0..feature_list.len() {
            let data = edges.entry(feature_list[i]).or_default();
            if i < feature_list.len() - 1 {
                data.insert(feature_list[i + 1]);
            }
        }
    }
    let mut discovered = BTreeSet::new();
    let mut currently_visiting = BTreeSet::new();
    let mut sorted_features = Vec::new();
    for &feature in edges.keys() {
        if !discovered.contains(&feature) && depth_first_search(&edges, &mut discovered, &mut currently_visiting, &mut sorted_features, feature) {
            panic!("Feature order cycle found");
        }
    }
    sorted_features.reverse();
    (0..max_step)
        .map(|step| StepFeatureData::new(sorted_features.iter().filter(|&&(s, _)| s == step).map(|&(_, index)| features_by_index[index]).collect()))
        .collect()
}

fn depth_first_search(
    edges: &BTreeMap<FeatureData, BTreeSet<FeatureData>>,
    discovered: &mut BTreeSet<FeatureData>,
    currently_visiting: &mut BTreeSet<FeatureData>,
    reverse_topological_order: &mut Vec<FeatureData>,
    current: FeatureData,
) -> bool {
    if discovered.contains(&current) {
        return false;
    }
    if currently_visiting.contains(&current) {
        return true;
    }
    currently_visiting.insert(current);
    if let Some(next) = edges.get(&current) {
        for &next in next {
            if depth_first_search(edges, discovered, currently_visiting, reverse_topological_order, next) {
                return true;
            }
        }
    }
    currently_visiting.remove(&current);
    discovered.insert(current);
    reverse_topological_order.push(current);
    false
}
