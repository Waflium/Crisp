use crisp_core::hash_map::FastMap;
use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;

use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;

pub type BlockInWorldPredicate = Box<dyn Fn(Option<BlockState>) -> bool + Send + Sync>;

pub struct BlockPattern {
    pattern: Vec<Vec<Vec<usize>>>,
    predicates: Vec<BlockInWorldPredicate>,
    depth: usize,
    height: usize,
    width: usize,
}

pub struct BlockPatternMatch {
    front_top_left: BlockPos,
    forwards: Direction,
    up: Direction,
    width: usize,
    height: usize,
    depth: usize,
}

impl BlockPatternMatch {
    pub fn get_front_top_left(&self) -> BlockPos {
        self.front_top_left
    }

    pub fn get_forwards(&self) -> Direction {
        self.forwards
    }

    pub fn get_up(&self) -> Direction {
        self.up
    }

    pub fn get_width(&self) -> usize {
        self.width
    }

    pub fn get_height(&self) -> usize {
        self.height
    }

    pub fn get_depth(&self) -> usize {
        self.depth
    }

    pub fn get_block(&self, right: i32, down: i32, forwards: i32) -> BlockPos {
        translate_and_rotate(self.front_top_left, self.forwards, self.up, right, down, forwards)
    }
}

struct BlockCache<'a> {
    level: &'a mut ServerLevel,
    load_chunks: bool,
    states: FastMap<BlockPos, Option<BlockState>>,
}

impl BlockCache<'_> {
    fn get_state(&mut self, pos: BlockPos) -> Option<BlockState> {
        if let Some(&state) = self.states.get(&pos) {
            return state;
        }
        let state = (self.load_chunks || self.level.has_chunk_at(pos)).then(|| self.level.block_state(pos));
        self.states.insert(pos, state);
        state
    }
}

impl BlockPattern {
    pub fn get_depth(&self) -> usize {
        self.depth
    }

    pub fn get_height(&self) -> usize {
        self.height
    }

    pub fn get_width(&self) -> usize {
        self.width
    }

    fn matches_in(&self, origin: BlockPos, forwards: Direction, up: Direction, cache: &mut BlockCache) -> Option<BlockPatternMatch> {
        for x in 0..self.width {
            for y in 0..self.height {
                for z in 0..self.depth {
                    let state = cache.get_state(translate_and_rotate(origin, forwards, up, x as i32, y as i32, z as i32));
                    if !(self.predicates[self.pattern[z][y][x]])(state) {
                        return None;
                    }
                }
            }
        }
        Some(BlockPatternMatch { front_top_left: origin, forwards, up, width: self.width, height: self.height, depth: self.depth })
    }

    pub fn matches(&self, level: &mut ServerLevel, origin: BlockPos, forwards: Direction, up: Direction) -> Option<BlockPatternMatch> {
        let mut cache = BlockCache { level, load_chunks: false, states: FastMap::default() };
        self.matches_in(origin, forwards, up, &mut cache)
    }

    pub fn find(&self, level: &mut ServerLevel, origin: BlockPos) -> Option<BlockPatternMatch> {
        let mut cache = BlockCache { level, load_chunks: false, states: FastMap::default() };
        let dist = self.width.max(self.height).max(self.depth) as i32;
        for test_pos in BlockPos::between_closed(origin, origin.offset(dist - 1, dist - 1, dist - 1)) {
            for forwards in Direction::VALUES {
                for up in Direction::VALUES {
                    if up != forwards && up != forwards.opposite() {
                        let found = self.matches_in(test_pos, forwards, up, &mut cache);
                        if found.is_some() {
                            return found;
                        }
                    }
                }
            }
        }
        None
    }
}

pub fn translate_and_rotate(origin: BlockPos, forwards_direction: Direction, up_direction: Direction, right: i32, down: i32, forwards: i32) -> BlockPos {
    assert!(forwards_direction != up_direction && forwards_direction != up_direction.opposite(), "Invalid forwards & up combination");
    let forwards_vector = (forwards_direction.step_x(), forwards_direction.step_y(), forwards_direction.step_z());
    let up_vector = (up_direction.step_x(), up_direction.step_y(), up_direction.step_z());
    let right_vector = (
        forwards_vector.1 * up_vector.2 - forwards_vector.2 * up_vector.1,
        forwards_vector.2 * up_vector.0 - forwards_vector.0 * up_vector.2,
        forwards_vector.0 * up_vector.1 - forwards_vector.1 * up_vector.0,
    );
    origin.offset(
        up_vector.0 * -down + right_vector.0 * right + forwards_vector.0 * forwards,
        up_vector.1 * -down + right_vector.1 * right + forwards_vector.1 * forwards,
        up_vector.2 * -down + right_vector.2 * right + forwards_vector.2 * forwards,
    )
}

pub struct BlockPatternBuilder {
    pattern: Vec<Vec<&'static str>>,
    lookup: Vec<(char, BlockInWorldPredicate)>,
}

impl BlockPatternBuilder {
    pub fn start() -> BlockPatternBuilder {
        BlockPatternBuilder { pattern: Vec::new(), lookup: vec![(' ', Box::new(|_| true))] }
    }

    pub fn aisle(mut self, aisle: &[&'static str]) -> BlockPatternBuilder {
        self.pattern.push(aisle.to_vec());
        self
    }

    pub fn where_(mut self, symbol: char, block_matcher: BlockInWorldPredicate) -> BlockPatternBuilder {
        self.lookup.push((symbol, block_matcher));
        self
    }

    pub fn build(self) -> BlockPattern {
        let depth = self.pattern.len();
        let height = self.pattern[0].len();
        let width = self.pattern[0][0].len();
        let index_of = |c: char| self.lookup.iter().position(|(symbol, _)| *symbol == c).unwrap_or_else(|| panic!("Predicates for character(s) {c} are missing"));
        let pattern = self.pattern.iter().map(|aisle| aisle.iter().map(|row| row.chars().map(index_of).collect()).collect()).collect();
        BlockPattern { pattern, predicates: self.lookup.into_iter().map(|(_, predicate)| predicate).collect(), depth, height, width }
    }
}

pub fn has_state(predicate: impl Fn(BlockState) -> bool + Send + Sync + 'static) -> BlockInWorldPredicate {
    Box::new(move |state| state.is_some_and(&predicate))
}
