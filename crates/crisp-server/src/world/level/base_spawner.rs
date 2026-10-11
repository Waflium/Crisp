use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

#[derive(Clone)]
pub struct BaseSpawner {
    spawn_delay: i32,
    spawn_potentials: Vec<(Nbt, i32)>,
    next_spawn_data: Option<Nbt>,
    min_spawn_delay: i32,
    max_spawn_delay: i32,
    spawn_count: i32,
    max_nearby_entities: i32,
    required_player_range: i32,
    spawn_range: i32,
}

impl Default for BaseSpawner {
    fn default() -> BaseSpawner {
        BaseSpawner {
            spawn_delay: 20,
            spawn_potentials: Vec::new(),
            next_spawn_data: None,
            min_spawn_delay: 200,
            max_spawn_delay: 800,
            spawn_count: 4,
            max_nearby_entities: 6,
            required_player_range: 16,
            spawn_range: 4,
        }
    }
}

fn new_spawn_data() -> Nbt {
    Nbt::Compound(vec![("entity".into(), Nbt::Compound(Vec::new()))])
}

fn is_spawn_data(tag: &Nbt) -> bool {
    matches!(tag.get("entity"), Some(Nbt::Compound(_)))
}

fn int_value(tag: &Nbt) -> Option<i32> {
    match tag {
        Nbt::Byte(v) => Some(*v as i32),
        Nbt::Short(v) => Some(*v as i32),
        Nbt::Int(v) => Some(*v),
        Nbt::Long(v) => Some(*v as i32),
        Nbt::Float(v) => Some(*v as i32),
        Nbt::Double(v) => Some(*v as i32),
        _ => None,
    }
}

impl BaseSpawner {
    pub fn set_entity_id(&mut self, entity_type: &str, random: &mut impl RandomSource) {
        let spawn_data = self.get_or_create_next_spawn_data(random);
        if let Nbt::Compound(fields) = spawn_data
            && let Some((_, Nbt::Compound(entity))) = fields.iter_mut().find(|(key, _)| key == "entity")
        {
            entity.retain(|(key, _)| key != "id");
            entity.push(("id".into(), Nbt::String(entity_type.into())));
        }
    }

    fn get_or_create_next_spawn_data(&mut self, random: &mut impl RandomSource) -> &mut Nbt {
        if self.next_spawn_data.is_none() {
            let total_weight: i32 = self.spawn_potentials.iter().map(|(_, weight)| *weight).sum();
            let picked = if total_weight == 0 {
                None
            } else {
                let mut selection = random.next_int_bounded(total_weight);
                self.spawn_potentials.iter().find(|(_, weight)| {
                    let found = selection < *weight;
                    selection -= weight;
                    found
                })
            };
            self.next_spawn_data = Some(picked.map_or_else(new_spawn_data, |(data, _)| data.clone()));
        }
        self.next_spawn_data.as_mut().expect("spawn data")
    }

    pub fn load(&mut self, input: &Nbt) {
        let int = |key: &str, default: i32| input.get(key).and_then(int_value).unwrap_or(default);
        self.spawn_delay = input.get("Delay").and_then(int_value).map_or(20, |v| v as i16 as i32);
        if let Some(spawn_data) = input.get("SpawnData").filter(|tag| is_spawn_data(tag)) {
            self.next_spawn_data = Some(spawn_data.clone());
        }
        self.spawn_potentials = match input.get("SpawnPotentials") {
            Some(Nbt::List(entries)) => entries
                .iter()
                .filter_map(|entry| Some((entry.get("data").filter(|data| is_spawn_data(data))?.clone(), entry.get("weight").and_then(int_value)?)))
                .collect(),
            _ => vec![(self.next_spawn_data.clone().unwrap_or_else(new_spawn_data), 1)],
        };
        self.min_spawn_delay = int("MinSpawnDelay", 200);
        self.max_spawn_delay = int("MaxSpawnDelay", 800);
        self.spawn_count = int("SpawnCount", 4);
        self.max_nearby_entities = int("MaxNearbyEntities", 6);
        self.required_player_range = int("RequiredPlayerRange", 16);
        self.spawn_range = int("SpawnRange", 4);
    }

    pub fn save(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("Delay".into(), Nbt::Short(self.spawn_delay as i16)));
        output.push(("MinSpawnDelay".into(), Nbt::Short(self.min_spawn_delay as i16)));
        output.push(("MaxSpawnDelay".into(), Nbt::Short(self.max_spawn_delay as i16)));
        output.push(("SpawnCount".into(), Nbt::Short(self.spawn_count as i16)));
        output.push(("MaxNearbyEntities".into(), Nbt::Short(self.max_nearby_entities as i16)));
        output.push(("RequiredPlayerRange".into(), Nbt::Short(self.required_player_range as i16)));
        output.push(("SpawnRange".into(), Nbt::Short(self.spawn_range as i16)));
        if let Some(spawn_data) = &self.next_spawn_data {
            output.push(("SpawnData".into(), spawn_data.clone()));
        }
        let potentials = self.spawn_potentials.iter().map(|(data, weight)| Nbt::Compound(vec![("data".into(), data.clone()), ("weight".into(), Nbt::Int(*weight))])).collect();
        output.push(("SpawnPotentials".into(), Nbt::List(potentials)));
    }
}
