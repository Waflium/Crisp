use serde_json::Value;

use super::structure_processor::StructureProcessorListHolder;
use super::structure_template_manager;

#[test]
fn every_processor_list_parses() {
    for (name, _) in crisp_data::PROCESSOR_LISTS {
        StructureProcessorListHolder::parse(&Value::String((*name).to_owned())).unwrap().value();
    }
}

#[test]
fn every_structure_template_loads() {
    for (name, _) in crisp_data::STRUCTURES {
        let size = structure_template_manager::get_or_create(name).get_size();
        assert!(size.x >= 1 && size.y >= 1 && size.z >= 1, "{name} has size {size:?}");
    }
}
