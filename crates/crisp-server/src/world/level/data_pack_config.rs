use crisp_protocol::nbt::Nbt;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DataPackConfig {
    pub enabled: Vec<String>,
    pub disabled: Vec<String>,
}

impl Default for DataPackConfig {
    fn default() -> DataPackConfig {
        DataPackConfig { enabled: vec!["vanilla".to_string()], disabled: Vec::new() }
    }
}

impl DataPackConfig {
    pub fn parse(nbt: &Nbt) -> Option<DataPackConfig> {
        let strings = |key: &str| -> Option<Vec<String>> {
            match nbt.get(key)? {
                Nbt::List(items) => items.iter().map(|item| item.as_str().map(str::to_string)).collect(),
                _ => None,
            }
        };
        Some(DataPackConfig { enabled: strings("Enabled")?, disabled: strings("Disabled")? })
    }

    pub fn to_nbt(&self) -> Nbt {
        let strings = |values: &[String]| Nbt::List(values.iter().map(|v| Nbt::String(v.clone())).collect());
        Nbt::Compound(vec![("Enabled".to_string(), strings(&self.enabled)), ("Disabled".to_string(), strings(&self.disabled))])
    }
}
