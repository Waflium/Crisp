use crisp_core::fastutil::{Long2ObjectOpenHashMap, LongOpenHashSet};
use crisp_core::ChunkPos;
use crisp_protocol::nbt::Nbt;

use super::chunk_level::{self, FullChunkStatus};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TicketType {
    PlayerSpawn,
    SpawnSearch,
    Dragon,
    PlayerLoading,
    PlayerSimulation,
    Forced,
    Portal,
    EnderPearl,
    Unknown,
}

const FLAG_PERSIST: i32 = 1;
const FLAG_LOADING: i32 = 2;
const FLAG_SIMULATION: i32 = 4;
const FLAG_KEEP_DIMENSION_ACTIVE: i32 = 8;
const FLAG_CAN_EXPIRE_IF_UNLOADED: i32 = 16;

impl TicketType {
    const VALUES: [TicketType; 9] = [
        TicketType::PlayerSpawn,
        TicketType::SpawnSearch,
        TicketType::Dragon,
        TicketType::PlayerLoading,
        TicketType::PlayerSimulation,
        TicketType::Forced,
        TicketType::Portal,
        TicketType::EnderPearl,
        TicketType::Unknown,
    ];

    fn name(self) -> &'static str {
        match self {
            TicketType::PlayerSpawn => "minecraft:player_spawn",
            TicketType::SpawnSearch => "minecraft:spawn_search",
            TicketType::Dragon => "minecraft:dragon",
            TicketType::PlayerLoading => "minecraft:player_loading",
            TicketType::PlayerSimulation => "minecraft:player_simulation",
            TicketType::Forced => "minecraft:forced",
            TicketType::Portal => "minecraft:portal",
            TicketType::EnderPearl => "minecraft:ender_pearl",
            TicketType::Unknown => "minecraft:unknown",
        }
    }

    fn by_name(name: &str) -> Option<TicketType> {
        let name = if name.contains(':') { name.to_string() } else { format!("minecraft:{name}") };
        TicketType::VALUES.into_iter().find(|t| t.name() == name)
    }

    pub fn timeout(self) -> i64 {
        match self {
            TicketType::PlayerSpawn => 20,
            TicketType::SpawnSearch | TicketType::Unknown => 1,
            TicketType::Portal => 300,
            TicketType::EnderPearl => 40,
            TicketType::Dragon | TicketType::PlayerLoading | TicketType::PlayerSimulation | TicketType::Forced => 0,
        }
    }

    fn flags(self) -> i32 {
        match self {
            TicketType::PlayerSpawn | TicketType::SpawnSearch | TicketType::PlayerLoading => 2,
            TicketType::Dragon => 6,
            TicketType::PlayerSimulation => 12,
            TicketType::Forced | TicketType::Portal => 15,
            TicketType::EnderPearl => 14,
            TicketType::Unknown => 18,
        }
    }

    pub fn persist(self) -> bool {
        self.flags() & FLAG_PERSIST != 0
    }

    pub fn does_load(self) -> bool {
        self.flags() & FLAG_LOADING != 0
    }

    pub fn does_simulate(self) -> bool {
        self.flags() & FLAG_SIMULATION != 0
    }

    pub fn should_keep_dimension_active(self) -> bool {
        self.flags() & FLAG_KEEP_DIMENSION_ACTIVE != 0
    }

    pub fn can_expire_if_unloaded(self) -> bool {
        self.flags() & FLAG_CAN_EXPIRE_IF_UNLOADED != 0
    }

    pub fn has_timeout(self) -> bool {
        self.timeout() != 0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ticket {
    pub ty: TicketType,
    pub level: i32,
    ticks_left: i64,
}

impl Ticket {
    pub fn new(ty: TicketType, level: i32) -> Ticket {
        Ticket { ty, level, ticks_left: ty.timeout() }
    }

    fn reset_ticks_left(&mut self) {
        self.ticks_left = self.ty.timeout();
    }

    fn decrease_ticks_left(&mut self) {
        if self.ty.has_timeout() {
            self.ticks_left -= 1;
        }
    }

    fn is_timed_out(&self) -> bool {
        self.ty.has_timeout() && self.ticks_left < 0
    }

    fn same(&self, other: &Ticket) -> bool {
        self.ty == other.ty && self.level == other.level
    }

    fn to_nbt(&self, pos: i64) -> Nbt {
        let pos = ChunkPos::unpack(pos);
        let mut fields = vec![
            ("chunk_pos".to_string(), Nbt::IntArray(vec![pos.x, pos.z])),
            ("type".into(), Nbt::String(self.ty.name().into())),
            ("level".into(), Nbt::Int(self.level)),
        ];
        if self.ticks_left != 0 {
            fields.push(("ticks_left".into(), Nbt::Long(self.ticks_left)));
        }
        Nbt::Compound(fields)
    }

    fn from_nbt(tag: &Nbt) -> Option<(i64, Ticket)> {
        let pos = match tag.get("chunk_pos")? {
            Nbt::IntArray(v) if v.len() == 2 => ChunkPos::new(v[0], v[1]),
            _ => return None,
        };
        let ty = TicketType::by_name(tag.get("type")?.as_str()?)?;
        let level = Some(tag.get("level")?.as_i64()? as i32).filter(|&l| l >= 0)?;
        let ticks_left = match tag.get("ticks_left") {
            Some(t) => t.as_i64()?,
            None => 0,
        };
        Some((pos.pack(), Ticket { ty, level, ticks_left }))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TicketUpdate {
    pub simulation: bool,
    pub node: i64,
    pub level: i32,
    pub only_decreased: bool,
}

pub struct TicketStorage {
    tickets: Long2ObjectOpenHashMap<Vec<Ticket>>,
    deactivated_tickets: Long2ObjectOpenHashMap<Vec<Ticket>>,
    chunks_with_forced_tickets: LongOpenHashSet,
    pub dirty: bool,
}

impl Default for TicketStorage {
    fn default() -> Self {
        TicketStorage {
            tickets: Long2ObjectOpenHashMap::with_expected(4, 0.75),
            deactivated_tickets: Long2ObjectOpenHashMap::new(),
            chunks_with_forced_tickets: LongOpenHashSet::new(),
            dirty: false,
        }
    }
}

const NO_TICKET_LEVEL: i32 = chunk_level::MAX_LEVEL + 1;

fn lowest_ticket(tickets: &[Ticket], simulation: bool) -> Option<&Ticket> {
    let mut lowest: Option<&Ticket> = None;
    for ticket in tickets {
        if lowest.is_none_or(|t| ticket.level < t.level) && if simulation { ticket.ty.does_simulate() } else { ticket.ty.does_load() } {
            lowest = Some(ticket);
        }
    }
    lowest
}

fn ticket_level_at(tickets: &[Ticket], simulation: bool) -> i32 {
    lowest_ticket(tickets, simulation).map_or(NO_TICKET_LEVEL, |t| t.level)
}

impl TicketStorage {
    pub fn from_nbt(data: &Nbt) -> TicketStorage {
        let mut storage = TicketStorage::default();
        for entry in data.get("tickets").map_or(&[][..], Nbt::as_list) {
            if let Some((pos, ticket)) = Ticket::from_nbt(entry) {
                storage.deactivated_tickets.get_or_insert_with(pos, || Vec::with_capacity(4)).push(ticket);
            }
        }
        storage
    }

    pub fn to_nbt(&self) -> Nbt {
        let tickets: Vec<Nbt> = [&self.tickets, &self.deactivated_tickets]
            .into_iter()
            .flat_map(|map| map.iter().flat_map(|(pos, tickets)| tickets.iter().filter(|t| t.ty.persist()).map(move |t| t.to_nbt(pos))))
            .collect();
        Nbt::Compound(if tickets.is_empty() { Vec::new() } else { vec![("tickets".into(), Nbt::List(tickets))] })
    }

    pub fn activate_all_deactivated_tickets(&mut self, updates: &mut Vec<TicketUpdate>) {
        let deactivated = std::mem::replace(&mut self.deactivated_tickets, Long2ObjectOpenHashMap::new());
        for (pos, tickets) in deactivated.iter() {
            for &ticket in tickets {
                self.add_ticket(pos, ticket, updates);
            }
        }
    }

    pub fn deactivate_tickets_on_closing(&mut self, updates: &mut Vec<TicketUpdate>) {
        let mut removed = std::mem::replace(&mut self.deactivated_tickets, Long2ObjectOpenHashMap::new());
        self.remove_ticket_if(|ticket, _| ticket.ty != TicketType::Unknown, Some(&mut removed), updates);
        self.deactivated_tickets = removed;
    }

    pub fn force_loaded_chunks(&self) -> &LongOpenHashSet {
        &self.chunks_with_forced_tickets
    }

    pub fn update_chunk_forced(&mut self, pos: ChunkPos, forced: bool, updates: &mut Vec<TicketUpdate>) -> bool {
        let ticket = Ticket::new(TicketType::Forced, chunk_level::by_full_status(FullChunkStatus::EntityTicking));
        if forced { self.add_ticket(pos.pack(), ticket, updates) } else { self.remove_ticket(pos.pack(), ticket, updates) }
    }

    fn update_forced_chunks(&mut self) {
        let mut chunks = LongOpenHashSet::new();
        for (pos, tickets) in self.tickets.iter() {
            if tickets.iter().any(|t| t.ty == TicketType::Forced) {
                chunks.add(pos);
            }
        }
        self.chunks_with_forced_tickets = chunks;
    }

    pub fn has_tickets(&self) -> bool {
        !self.tickets.is_empty()
    }

    pub fn should_keep_dimension_active(&self) -> bool {
        self.tickets.iter().any(|(_, tickets)| tickets.iter().any(|t| t.ty.should_keep_dimension_active()))
    }

    pub fn iter(&self) -> impl Iterator<Item = (i64, &[Ticket])> + '_ {
        self.tickets.iter().map(|(key, tickets)| (key, tickets.as_slice()))
    }

    pub fn tickets(&self, key: i64) -> &[Ticket] {
        self.tickets.get(key).map_or(&[], Vec::as_slice)
    }

    pub fn add_ticket_with_radius(&mut self, ty: TicketType, key: i64, radius: i32, updates: &mut Vec<TicketUpdate>) {
        let ticket = Ticket::new(ty, chunk_level::by_full_status(FullChunkStatus::Full) - radius);
        self.add_ticket(key, ticket, updates);
    }

    pub fn remove_ticket_with_radius(&mut self, ty: TicketType, key: i64, radius: i32, updates: &mut Vec<TicketUpdate>) {
        let ticket = Ticket::new(ty, chunk_level::by_full_status(FullChunkStatus::Full) - radius);
        self.remove_ticket(key, ticket, updates);
    }

    pub fn add_ticket(&mut self, key: i64, ticket: Ticket, updates: &mut Vec<TicketUpdate>) -> bool {
        let tickets = self.tickets.get_or_insert_with(key, || Vec::with_capacity(4));
        if let Some(existing) = tickets.iter_mut().find(|t| t.same(&ticket)) {
            existing.reset_ticks_left();
            self.dirty = true;
            return false;
        }
        let old_simulation = ticket_level_at(tickets, true);
        let old_loading = ticket_level_at(tickets, false);
        tickets.push(ticket);
        if ticket.ty.does_simulate() && ticket.level < old_simulation {
            updates.push(TicketUpdate { simulation: true, node: key, level: ticket.level, only_decreased: true });
        }
        if ticket.ty.does_load() && ticket.level < old_loading {
            updates.push(TicketUpdate { simulation: false, node: key, level: ticket.level, only_decreased: true });
        }
        if ticket.ty == TicketType::Forced {
            self.chunks_with_forced_tickets.add(key);
        }
        self.dirty = true;
        true
    }

    pub fn ticket_level_at(&self, key: i64, simulation: bool) -> i32 {
        ticket_level_at(self.tickets(key), simulation)
    }

    pub fn remove_ticket(&mut self, key: i64, ticket: Ticket, updates: &mut Vec<TicketUpdate>) -> bool {
        let Some(tickets) = self.tickets.get_mut(key) else { return false };
        let Some(index) = tickets.iter().position(|t| t.same(&ticket)) else { return false };
        tickets.remove(index);
        let (simulation, loading) = (ticket_level_at(tickets, true), ticket_level_at(tickets, false));
        if tickets.is_empty() {
            self.tickets.remove(key);
        }
        if ticket.ty.does_simulate() {
            updates.push(TicketUpdate { simulation: true, node: key, level: simulation, only_decreased: false });
        }
        if ticket.ty.does_load() {
            updates.push(TicketUpdate { simulation: false, node: key, level: loading, only_decreased: false });
        }
        if ticket.ty == TicketType::Forced {
            self.update_forced_chunks();
        }
        self.dirty = true;
        true
    }

    pub fn purge_stale_tickets(&mut self, may_expire_loaded: impl Fn(i64) -> bool, updates: &mut Vec<TicketUpdate>) {
        self.remove_ticket_if(
            |ticket, pos| {
                let can_expire = ticket.ty.has_timeout() && (ticket.ty.can_expire_if_unloaded() || may_expire_loaded(pos));
                if can_expire {
                    ticket.decrease_ticks_left();
                    ticket.is_timed_out()
                } else {
                    false
                }
            },
            None,
            updates,
        );
        self.dirty = true;
    }

    fn remove_ticket_if(
        &mut self,
        mut predicate: impl FnMut(&mut Ticket, i64) -> bool,
        mut removed: Option<&mut Long2ObjectOpenHashMap<Vec<Ticket>>>,
        updates: &mut Vec<TicketUpdate>,
    ) {
        let (mut removed_forced, mut dirty) = (false, false);
        self.tickets.retain(|pos, tickets| {
            let (mut removed_simulation, mut removed_loading) = (false, false);
            let capacity = tickets.len();
            tickets.retain_mut(|ticket| {
                if !predicate(ticket, pos) {
                    return true;
                }
                if let Some(removed) = removed.as_deref_mut() {
                    removed.get_or_insert_with(pos, || Vec::with_capacity(capacity)).push(*ticket);
                }
                removed_loading |= ticket.ty.does_load();
                removed_simulation |= ticket.ty.does_simulate();
                removed_forced |= ticket.ty == TicketType::Forced;
                false
            });
            if removed_loading || removed_simulation {
                if removed_loading {
                    updates.push(TicketUpdate { simulation: false, node: pos, level: ticket_level_at(tickets, false), only_decreased: false });
                }
                if removed_simulation {
                    updates.push(TicketUpdate { simulation: true, node: pos, level: ticket_level_at(tickets, true), only_decreased: false });
                }
                dirty = true;
                !tickets.is_empty()
            } else {
                true
            }
        });
        self.dirty |= dirty;
        if removed_forced {
            self.update_forced_chunks();
        }
    }

    pub fn replace_ticket_level_of_type(&mut self, new_level: i32, ty: TicketType, updates: &mut Vec<TicketUpdate>) {
        let mut affected = Vec::new();
        for (pos, tickets) in self.tickets.iter() {
            affected.extend(tickets.iter().filter(|t| t.ty == ty).map(|t| (*t, pos)));
        }
        for (ticket, pos) in affected {
            self.remove_ticket(pos, ticket, updates);
            self.add_ticket(pos, Ticket::new(ty, new_level), updates);
        }
    }
}
