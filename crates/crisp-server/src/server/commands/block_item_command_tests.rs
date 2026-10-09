use crisp_core::{BlockPos, ChunkPos, Vec3};
use crisp_data::properties::{HORIZONTAL_FACING, WATERLOGGED};
use crisp_data::{blocks, items};

use crate::commands::brigadier::{self, ContextChain, DISPATCHER_UNKNOWN_ARGUMENT, DISPATCHER_UNKNOWN_COMMAND, StringReader};
use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::nbt::tests::error;
use crate::network::login::{Profile, offline_uuid};
use crate::server::Server;
use crate::server::level::game_mode::GameType;
use crate::server::level::testing;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;

fn server() -> Server {
    let mut server = Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap();
    for x in -1..=1 {
        for z in -1..=1 {
            server.level.chunks.chunk(ChunkPos::new(x, z));
        }
    }
    server
}

fn run(server: &mut Server, command: &str) -> Vec<String> {
    server.console_log.borrow_mut().clear();
    let source = CommandSourceStack::server(server);
    perform_prefixed_command(server, &source, command);
    server.console_log.borrow().clone()
}

fn parse(server: &Server, command: &str) -> String {
    let units: Vec<u16> = command.encode_utf16().collect();
    let source = CommandSourceStack::server(server);
    let parse = brigadier::parse(StringReader::new(&units), &source);
    let e = if !parse.reader.can_read() {
        match ContextChain::try_flatten(parse.context.build(command)) {
            Some(_) => return "OK".into(),
            None => DISPATCHER_UNKNOWN_COMMAND.create_with_context(&parse.reader),
        }
    } else if parse.exceptions.len() == 1 {
        parse.exceptions[0].1.clone()
    } else if parse.context.range.is_empty() {
        DISPATCHER_UNKNOWN_COMMAND.create_with_context(&parse.reader)
    } else {
        DISPATCHER_UNKNOWN_ARGUMENT.create_with_context(&parse.reader)
    };
    error(&e)
}

fn join(server: &mut Server, name: &str) -> EntityId {
    let profile = Profile { uuid: offline_uuid(name), name: name.into(), properties: Vec::new() };
    testing::player_with_profile(&mut server.level, profile, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0)
}

fn block(server: &mut Server, x: i32, y: i32, z: i32) -> crisp_data::BlockState {
    server.level.block_state(BlockPos::new(x, y, z))
}

fn items_in_level(server: &Server) -> Vec<&crate::world::entity::ItemEntity> {
    server.level.entities.values().filter_map(LevelEntity::as_item).collect()
}

#[test]
fn arguments_parse_like_vanilla() {
    let server = server();
    let cases: &[(&str, &str)] = &[
        ("setblock 0 0 0 stone", "OK"),
        ("setblock 0 0 0 minecraft:stone", "OK"),
        ("setblock 0 0 0 nope", "ERR Unknown block type 'minecraft:nope' | CTX ...ock 0 0 0 nope<--[HERE]"),
        ("setblock 0 0 0 nope:stone", "ERR Unknown block type 'nope:stone' | CTX ...ock 0 0 0 nope:stone<--[HERE]"),
        ("setblock 0 0 0 Stone", "ERR Unknown block type 'minecraft:' | CTX ...ock 0 0 0 Stone<--[HERE]"),
        ("setblock 0 0 0 ..:stone", "ERR Invalid ID | CTX ...ock 0 0 0 ..:stone<--[HERE]"),
        ("setblock 0 0 0 stone[foo=bar]", "ERR Block minecraft:stone does not have property 'foo' | CTX ...0 0 stone[foo=bar]<--[HERE]"),
        ("setblock 0 0 0 oak_stairs[facing=north,facing=south]", "ERR Property 'facing' can only be set once for block minecraft:oak_stairs | CTX ...ing=north,facing=south]<--[HERE]"),
        ("setblock 0 0 0 oak_stairs[facing]", "ERR Expected value for property 'facing' on block minecraft:oak_stairs | CTX ...irs[facing]<--[HERE]"),
        ("setblock 0 0 0 oak_stairs[facing=]", "ERR Block minecraft:oak_stairs does not accept '' for facing property | CTX ...rs[facing=]<--[HERE]"),
        ("setblock 0 0 0 oak_stairs[facing=up]", "ERR Block minecraft:oak_stairs does not accept 'up' for facing property | CTX ...rs[facing=up]<--[HERE]"),
        ("setblock 0 0 0 oak_stairs[facing=north", "ERR Expected closing ] for block state properties | CTX ...cing=north<--[HERE]"),
        ("setblock 0 0 0 oak_stairs[facing=north;]", "ERR Expected closing ] for block state properties | CTX ...cing=north;]<--[HERE]"),
        ("setblock 0 0 0 oak_stairs[facing=north,]", "OK"),
        ("setblock 0 0 0 oak_stairs[]", "OK"),
        ("setblock 0 0 0 oak_stairs[", "ERR Expected closing ] for block state properties | CTX ...ak_stairs[<--[HERE]"),
        ("setblock 0 0 0 oak_stairs[ facing = north , half = top ]", "OK"),
        ("setblock 0 0 0 oak_stairs[\"facing\"=\"north\"]", "OK"),
        ("setblock 0 0 0 oak_stairs[facing=\"north]", "ERR Unclosed quoted string | CTX ...ng=\"north]<--[HERE]"),
        ("setblock 0 0 0 wheat[age=+7]", "OK"),
        ("setblock 0 0 0 wheat[age=07]", "OK"),
        ("setblock 0 0 0 wheat[age=8]", "ERR Block minecraft:wheat does not accept '8' for age property | CTX ...wheat[age=8]<--[HERE]"),
        ("setblock 0 0 0 wheat[age=-0]", "OK"),
        ("setblock 0 0 0 wheat[age=99999999999]", "ERR Block minecraft:wheat does not accept '99999999999' for age property | CTX ...wheat[age=99999999999]<--[HERE]"),
        ("setblock 0 0 0 oak_stairs[waterlogged=TRUE]", "ERR Block minecraft:oak_stairs does not accept 'TRUE' for waterlogged property | CTX ...terlogged=TRUE]<--[HERE]"),
        ("setblock 0 0 0 chest{}", "OK"),
        ("setblock 0 0 0 chest{a:1}", "OK"),
        ("setblock 0 0 0 chest[facing=east]{Items:[]}", "OK"),
        ("setblock 0 0 0 chest{a:}", "ERR Expected a valid unquoted string | CTX ...0 chest{a:}<--[HERE]"),
        ("setblock 0 0 0 chest[]{", "ERR Expected literal \" | CTX ...0 chest[]{<--[HERE]"),
        ("setblock 0 0 0 stone{}x", "ERR Expected whitespace to end one argument, but found trailing data | CTX ... 0 stone{}x<--[HERE]"),
        ("setblock 0 0 0 #minecraft:logs", "ERR Tags aren't allowed here; only actual blocks are | CTX ...ock 0 0 0 #minecraft:logs<--[HERE]"),
        ("setblock 0 0 0 stone destroy", "OK"),
        ("setblock 0 0 0 stone foo", "ERR Incorrect argument for command | CTX ...0 0 stone foo<--[HERE]"),
        ("fill 0 0 0 1 1 1 stone replace #minecraft:logs[axis=y]", "OK"),
        ("fill 0 0 0 1 1 1 stone replace #minecraft:logs[axis=y,axis=x]", "ERR Property 'axis' can only be set once for block minecraft: | CTX ...gs[axis=y,axis=x]<--[HERE]"),
        ("fill 0 0 0 1 1 1 stone replace #minecraft:logs[axis]", "ERR Expected value for property 'axis' on block minecraft: | CTX ...raft:logs[axis]<--[HERE]"),
        ("fill 0 0 0 1 1 1 stone replace #minecraft:logs[axis=", "ERR Expected closing ] for block state properties | CTX ...logs[axis=<--[HERE]"),
        ("fill 0 0 0 1 1 1 stone replace #minecraft:logs[axis=y", "ERR Expected closing ] for block state properties | CTX ...logs[axis=y<--[HERE]"),
        ("fill 0 0 0 1 1 1 stone replace #minecraft:logs[axis=y x]", "ERR Expected closing ] for block state properties | CTX ...gs[axis=y x]<--[HERE]"),
        ("fill 0 0 0 1 1 1 stone replace #minecraft:logs[foo=bar]", "OK"),
        ("fill 0 0 0 1 1 1 stone replace #minecraft:logs{a:1}", "OK"),
        ("fill 0 0 0 1 1 1 stone replace stone[foo=bar]", "ERR Block minecraft:stone does not have property 'foo' | CTX ...ace stone[foo=bar]<--[HERE]"),
        ("fill 0 0 0 1 1 1 stone replace stone hollow", "OK"),
        ("fill 0 0 0 1 1 1 stone keep", "OK"),
        ("fill 0 0 0 1 1 1 stone outline", "OK"),
        ("clone 0 0 0 1 1 1 2 2 2 filtered #minecraft:logs[axis=y] move", "OK"),
        ("clone 0 0 0 1 1 1 2 2 2 strict masked force", "OK"),
        ("clone from minecraft:the_nether 0 0 0 1 1 1 2 2 2", "OK"),
        ("clone 0 0 0 1 1 1 to minecraft:overworld 2 2 2 strict", "OK"),
        ("give Steve diamond_sword[damage=\"a\"]", "ERR Malformed 'minecraft:damage' component: 'Not a number' | CTX ...rd[damage=\"a\"]<--[HERE]"),
        ("give Steve diamond_sword[damage={}]", "ERR Malformed 'minecraft:damage' component: 'Not a number' | CTX ...rd[damage={}]<--[HERE]"),
        ("give Steve diamond_sword[damage=1.5]", "OK"),
        ("give Steve diamond_sword[damage=3000000000L]", "ERR Malformed 'minecraft:damage' component: 'Value must be non-negative: -1294967296' | CTX ...rd[damage=3000000000L]<--[HERE]"),
        ("give Steve diamond_sword[damage=true]", "OK"),
        ("give Steve diamond_sword[damage=[1]]", "ERR Malformed 'minecraft:damage' component: 'Not a number' | CTX ...rd[damage=[1]]<--[HERE]"),
        ("give Steve diamond_sword[damage=-1]", "ERR Malformed 'minecraft:damage' component: 'Value must be non-negative: -1' | CTX ...rd[damage=-1]<--[HERE]"),
        ("give Steve stone[max_stack_size=100]", "ERR Malformed 'minecraft:max_stack_size' component: 'Value must be within range [1;99]: 100' | CTX ...tack_size=100]<--[HERE]"),
        ("give Steve stone[max_stack_size=0b]", "ERR Malformed 'minecraft:max_stack_size' component: 'Value must be within range [1;99]: 0' | CTX ...tack_size=0b]<--[HERE]"),
        ("give Steve stone[max_damage=0]", "ERR Malformed 'minecraft:max_damage' component: 'Value must be positive: 0' | CTX ...ax_damage=0]<--[HERE]"),
        ("give Steve stone[unbreakable=1]", "ERR Malformed 'minecraft:unbreakable' component: 'Not a map: 1' | CTX ...breakable=1]<--[HERE]"),
        ("give Steve stone[unbreakable={}]", "OK"),
        ("give Steve stone[unbreakable={a:1}]", "OK"),
        ("give Steve stone[unbreakable=\"x\"]", "ERR Malformed 'minecraft:unbreakable' component: 'Not a map: \"x\"' | CTX ...breakable=\"x\"]<--[HERE]"),
        ("give Steve stone[rarity=\"foo\"]", "ERR Malformed 'minecraft:rarity' component: 'Unknown element name:foo' | CTX ...ne[rarity=\"foo\"]<--[HERE]"),
        ("give Steve stone[rarity=1]", "ERR Malformed 'minecraft:rarity' component: 'Not a string' | CTX ...ne[rarity=1]<--[HERE]"),
        ("give Steve stone[rarity=epic]", "OK"),
        ("give Steve stone[enchantment_glint_override=1b]", "OK"),
        ("give Steve stone[enchantment_glint_override=\"x\"]", "ERR Malformed 'minecraft:enchantment_glint_override' component: 'Not a number' | CTX ..._override=\"x\"]<--[HERE]"),
        ("give Steve stone[repair_cost=-1]", "ERR Malformed 'minecraft:repair_cost' component: 'Value must be non-negative: -1' | CTX ...pair_cost=-1]<--[HERE]"),
        ("give Steve stone[custom_name=1]", "ERR Malformed 'minecraft:custom_name' component: 'Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: 1; Second: Not a map: 1' | CTX ...stom_name=1]<--[HERE]"),
        ("give Steve stone[custom_name=1.5f]", "ERR Malformed 'minecraft:custom_name' component: 'Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: 1.5f; Second: Not a map: 1.5f' | CTX ...stom_name=1.5f]<--[HERE]"),
        ("give Steve stone[custom_name=\"x\"]", "OK"),
        ("give Steve stone[creative_slot_lock={}]", "ERR Unknown item component 'minecraft:creative_slot_lock' | CTX ...eve stone[creative_slot_lock={}]<--[HERE]"),
        ("give Steve stone[damage=1,damage=2]", "ERR Item component 'minecraft:damage' was repeated, but only one value can be specified"),
        ("give Steve stone[!damage,damage=2]", "ERR Item component 'minecraft:damage' was repeated, but only one value can be specified"),
        ("give Steve stone[damage=1,]", "OK"),
        ("give Steve stone[damage=1", "ERR Expected ']' | CTX ...e[damage=1<--[HERE]"),
        ("give Steve stone[damage=1,", "ERR Expected item component | CTX ...[damage=1,<--[HERE]"),
        ("give Steve stone[damage=1, ]", "OK"),
        ("give Steve stone[]", "OK"),
        ("give Steve stone[", "ERR Expected ']' | CTX ...eve stone[<--[HERE]"),
        ("give Steve stone[!]", "ERR Unknown item component 'minecraft:' | CTX ...ve stone[!]<--[HERE]"),
        ("give Steve stone[ damage = 1 ]", "OK"),
        ("give Steve stone[damage 1]", "ERR Expected '=' | CTX ...ne[damage 1]<--[HERE]"),
        ("give Steve stone[foo=1]", "ERR Unknown item component 'minecraft:foo' | CTX ...eve stone[foo=1]<--[HERE]"),
        ("give Steve stone[Damage=1]", "ERR Unknown item component 'minecraft:' | CTX ...eve stone[Damage=1]<--[HERE]"),
        ("give Steve stone[damage=}]", "ERR Expected a valid unquoted string | CTX ...ne[damage=}]<--[HERE]"),
        ("give Steve stone[!lore]", "OK"),
        ("give Steve nope", "ERR Unknown item 'minecraft:nope' | CTX ...ive Steve nope<--[HERE]"),
        ("give Steve stone 0", "ERR Integer must not be less than 1: found 0 | CTX ...eve stone 0<--[HERE]"),
        ("give Steve stone 5", "OK"),
        ("give Steve stone{}", "ERR Expected whitespace to end one argument, but found trailing data | CTX ...teve stone{}<--[HERE]"),
        ("clear Steve *[count=5]", "OK"),
        ("clear Steve *[count=\"x\"]", "ERR Malformed 'minecraft:count' component: 'Failed to parse either. First: Not a map: \"x\"; Second: Not a number' | CTX ...[count=\"x\"]<--[HERE]"),
        ("clear Steve *[count={min:\"x\"}]", "ERR Malformed 'minecraft:count' component: 'Not a number' | CTX ...={min:\"x\"}]<--[HERE]"),
        ("clear Steve *[count={min:3,max:1}]", "ERR Malformed 'minecraft:count' component: 'Swapped bounds in range: Optional[3] is higher than Optional[1]' | CTX ...n:3,max:1}]<--[HERE]"),
        ("clear Steve *[count=1.5]", "OK"),
        ("clear Steve *[count={}]", "OK"),
        ("clear Steve *[count~\"x\"]", "ERR Malformed 'minecraft:count' predicate: 'Failed to parse either. First: Not a map: \"x\"; Second: Not a number' | CTX ...[count~\"x\"]<--[HERE]"),
        ("clear Steve *[damage~{damage:5}]", "OK"),
        ("clear Steve *[damage~{damage:\"x\"}]", "ERR Malformed 'minecraft:damage' predicate: 'Failed to parse either. First: Not a map: \"x\"; Second: Not a number' | CTX ...amage:\"x\"}]<--[HERE]"),
        ("clear Steve *[damage~5]", "ERR Malformed 'minecraft:damage' predicate: 'Not a map: 5' | CTX ...*[damage~5]<--[HERE]"),
        ("clear Steve *[damage~{durability:{min:3,max:1}}]", "ERR Malformed 'minecraft:damage' predicate: 'Swapped bounds in range: Optional[3] is higher than Optional[1]' | CTX ...:3,max:1}}]<--[HERE]"),
        ("clear Steve *[unbreakable~{}]", "OK"),
        ("clear Steve *[unbreakable~1]", "ERR Malformed 'minecraft:unbreakable' predicate: 'Not a map: 1' | CTX ...reakable~1]<--[HERE]"),
        ("clear Steve *[creative_slot_lock~{}]", "OK"),
        ("clear Steve *[creative_slot_lock]", "ERR Expected literal ~ | CTX ..._slot_lock]<--[HERE]"),
        ("clear Steve *[foo]", "ERR Unknown item component 'minecraft:foo' | CTX ...teve *[foo]<--[HERE]"),
        ("clear Steve *[foo~1]", "ERR Unknown item component 'minecraft:foo' | CTX ...teve *[foo~1]<--[HERE]"),
        ("clear Steve *[damage]", "OK"),
        ("clear Steve *[damage=]", "ERR Expected a valid unquoted string | CTX ... *[damage=]<--[HERE]"),
        ("clear Steve *[damage=x]", "ERR Malformed 'minecraft:damage' component: 'Not a number' | CTX ...*[damage=x]<--[HERE]"),
        ("clear Steve *[damage=1,]", "ERR Invalid ID | CTX ...[damage=1,]<--[HERE]"),
        ("clear Steve *[damage=1|]", "ERR Invalid ID | CTX ...[damage=1|]<--[HERE]"),
        ("clear Steve *[!damage]", "OK"),
        ("clear Steve *[!]", "ERR Invalid ID | CTX ... Steve *[!]<--[HERE]"),
        ("clear Steve *[]", "OK"),
        ("clear Steve *[", "ERR Invalid ID | CTX ...r Steve *[<--[HERE]"),
        ("clear Steve stone[", "ERR Invalid ID | CTX ...eve stone[<--[HERE]"),
        ("clear Steve #minecraft:logs[damage=1]", "OK"),
        ("clear Steve #", "ERR Invalid ID | CTX ...ar Steve #<--[HERE]"),
        ("clear Steve nope", "ERR Unknown item 'minecraft:nope' | CTX ...Steve nope<--[HERE]"),
        ("clear Steve nope:x", "ERR Unknown item 'nope:x' | CTX ...eve nope:x<--[HERE]"),
        ("clear Steve Stone", "ERR Invalid ID | CTX ...ear Steve Stone<--[HERE]"),
        ("clear Steve *", "OK"),
        ("clear Steve **", "ERR Expected whitespace to end one argument, but found trailing data | CTX ...ar Steve **<--[HERE]"),
        ("clear Steve stone 5", "OK"),
        ("clear Steve stone -1", "ERR Integer must not be less than 0: found -1 | CTX ...eve stone -1<--[HERE]"),
        ("clear Steve stone[damage=1]x", "ERR Expected whitespace to end one argument, but found trailing data | CTX ...[damage=1]x<--[HERE]"),
        ("clear Steve stone[custom_name=1]", "ERR Malformed 'minecraft:custom_name' component: 'Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: 1; Second: Not a map: 1' | CTX ...tom_name=1]<--[HERE]"),
        ("clear Steve stone[ damage = 1 , !unbreakable | rarity = \"epic\" ]", "OK"),
        ("clear Steve stone[damage={a:]", "ERR Expected a valid unquoted string | CTX ...damage={a:]<--[HERE]"),
        ("clear Steve stone[minecraft:damage=1]", "OK"),
        ("clear Steve stone[Damage=1]", "ERR Invalid ID | CTX ...eve stone[Damage=1]<--[HERE]"),
        ("clear Steve stone[1]", "ERR Unknown item component 'minecraft:1' | CTX ...ve stone[1]<--[HERE]"),
        ("clear Steve stone[:]", "ERR Unknown item component 'minecraft:' | CTX ...ve stone[:]<--[HERE]"),
        ("clear Steve stone[!!damage]", "ERR Invalid ID | CTX ...ve stone[!!damage]<--[HERE]"),
        ("clear Steve stone[damage=1 damage=2]", "ERR Expected literal | | CTX ...damage=1 damage=2]<--[HERE]"),
        ("clear Steve minecraft:", "ERR Unknown item 'minecraft:' | CTX ...minecraft:<--[HERE]"),
        ("clear Steve :stone", "OK"),
        ("clear Steve stone{}", "ERR Expected whitespace to end one argument, but found trailing data | CTX ...teve stone{}<--[HERE]"),
        ("data get block 0 0 0 a.b[0].c", "OK"),
        ("data get block 0 0 0 a..b", "ERR Invalid NBT path element | CTX ...k 0 0 0 a..b<--[HERE]"),
        ("data get block 0 0 0 a[x]", "ERR Expected integer | CTX ...k 0 0 0 a[x]<--[HERE]"),
        ("data get block 0 0 0 a{b:1}[{c:2}]", "OK"),
        ("data get block 0 0 0 {a:1}", "OK"),
        ("data get block 0 0 0 a.{a:1}", "ERR Invalid NBT path element | CTX ...k 0 0 0 a.{a:1}<--[HERE]"),
        ("data get block 0 0 0 \"a b\".c", "OK"),
        ("data get block 0 0 0 \"\"", "ERR Invalid NBT path element | CTX ...k 0 0 0 \"\"<--[HERE]"),
        ("data get block 0 0 0 a[]", "OK"),
        ("data get block 0 0 0 a[1", "ERR Expected ']' | CTX ... 0 0 0 a[1<--[HERE]"),
        ("data get block 0 0 0 a[{}x]", "ERR Expected ']' | CTX ...0 0 0 a[{}x]<--[HERE]"),
        ("data merge block 0 0 0 {a:1}", "OK"),
        ("data merge block 0 0 0 [1]", "ERR Expected compound tag | CTX ... 0 0 0 [1]<--[HERE]"),
        ("data merge block 0 0 0 {a:", "ERR Expected a valid unquoted string | CTX ... 0 0 0 {a:<--[HERE]"),
        ("fill 0 0 0 1 1 1 stone replace #nope:x", "ERR Unknown block tag 'nope:x' | CTX ...e replace #nope:x<--[HERE]"),
        ("clear Steve #nope:x", "ERR Unknown item tag 'nope:x' | CTX ...ve #nope:x<--[HERE]"),
        ("clear Steve *[lore=[]]", "OK"),
        ("clear Steve *[enchantments~{}]", "ERR Could not parse command: Item predicate 'minecraft:enchantments' is not supported yet | CTX ...ntments~{}]<--[HERE]"),
    ];
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|(input, expected)| {
            let actual = parse(&server, input);
            (actual != *expected).then(|| format!("{input}\n  expected: {expected}\n  crisp:    {actual}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn setblock() {
    let mut server = server();
    assert_eq!(run(&mut server, "setblock 100 -60 0 stone"), ["That position is not loaded"]);
    assert_eq!(run(&mut server, "setblock 0 400 0 stone"), ["That position is out of this world!"]);
    assert_eq!(run(&mut server, "setblock 0 -60 0 oak_stairs[facing=east]"), ["Changed the block at 0, -60, 0"]);
    let stairs = block(&mut server, 0, -60, 0);
    assert!(stairs.is_block(blocks::OAK_STAIRS) && stairs.get(HORIZONTAL_FACING) == crisp_core::Direction::East);
    assert_eq!(run(&mut server, "setblock 0 -60 0 oak_stairs[facing=east]"), ["Could not set the block"]);
    assert_eq!(run(&mut server, "setblock 0 -60 0 stone keep"), ["Could not set the block"]);
    assert_eq!(run(&mut server, "setblock 1 -60 0 stone keep"), ["Changed the block at 1, -60, 0"]);
    assert_eq!(run(&mut server, "setblock 2 -61 0 air destroy"), ["Changed the block at 2, -61, 0"]);
    assert!(block(&mut server, 2, -61, 0).is_air());
    assert_eq!(items_in_level(&server).len(), 1);
    assert_eq!(run(&mut server, "setblock ~ ~-1 ~ glass strict"), ["Changed the block at 0, -61, 0"]);
}

#[test]
fn setblock_keeps_the_given_properties_against_shape_updates() {
    let mut server = server();
    run(&mut server, "setblock 0 -60 0 water");
    run(&mut server, "setblock 1 -60 0 oak_slab[waterlogged=false]");
    assert!(!block(&mut server, 1, -60, 0).get(WATERLOGGED));
}

#[test]
fn fill() {
    let mut server = server();
    assert_eq!(run(&mut server, "fill 0 -60 0 2 -58 2 stone"), ["Successfully filled 27 block(s)"]);
    assert_eq!(run(&mut server, "fill 0 -60 0 2 -58 2 stone"), ["No blocks were filled"]);
    assert_eq!(run(&mut server, "fill 0 -60 0 2 -58 2 dirt hollow"), ["Successfully filled 27 block(s)"]);
    assert!(block(&mut server, 1, -59, 1).is_air());
    assert_eq!(run(&mut server, "fill 0 -60 0 2 -58 2 glass outline"), ["Successfully filled 26 block(s)"]);
    assert!(block(&mut server, 1, -59, 1).is_air());
    assert_eq!(run(&mut server, "fill 0 -60 0 2 -58 2 stone keep"), ["Successfully filled 1 block(s)"]);
    assert_eq!(run(&mut server, "fill 0 -60 0 2 -58 2 oak_log replace stone"), ["Successfully filled 1 block(s)"]);
    assert_eq!(run(&mut server, "fill 0 -60 0 2 -58 2 dirt replace #minecraft:logs[axis=x]"), ["No blocks were filled"]);
    assert_eq!(run(&mut server, "fill 0 -60 0 2 -58 2 dirt replace #minecraft:logs[axis=y] outline"), ["No blocks were filled"]);
    assert_eq!(run(&mut server, "fill 0 -60 0 2 -58 2 dirt replace #minecraft:logs[axis=y]"), ["Successfully filled 1 block(s)"]);
    assert_eq!(run(&mut server, "fill 0 -60 0 2 -58 2 dirt replace chest{}"), ["No blocks were filled"]);
    assert_eq!(run(&mut server, "fill 0 -60 0 0 -60 0 glass destroy"), ["Successfully filled 1 block(s)"]);
    assert_eq!(run(&mut server, "fill -16 -60 -16 24 -20 24 stone"), ["Too many blocks in the specified area (maximum 32768, but specified 68921)"]);
    run(&mut server, "gamerule max_block_modifications 10");
    assert_eq!(run(&mut server, "fill 0 -60 0 2 -60 3 stone"), ["Too many blocks in the specified area (maximum 10, but specified 12)"]);
    assert_eq!(run(&mut server, "fill 0 -60 0 100 -60 0 stone"), ["That position is not loaded"]);
}

#[test]
fn clone() {
    let mut server = server();
    run(&mut server, "fill 0 -60 0 1 -60 1 stone");
    run(&mut server, "setblock 0 -60 0 oak_stairs");
    assert_eq!(run(&mut server, "clone 0 -60 0 1 -60 1 0 -55 0"), ["Successfully cloned 4 block(s)"]);
    assert!(block(&mut server, 0, -55, 0).is_block(blocks::OAK_STAIRS) && block(&mut server, 1, -55, 1).is_block(blocks::STONE));
    assert_eq!(run(&mut server, "clone 0 -60 0 1 -60 1 1 -60 1"), ["The source and destination areas cannot overlap"]);
    assert_eq!(run(&mut server, "clone 0 -60 0 1 -60 1 0 -55 0"), ["Successfully cloned 4 block(s)"]);
    assert_eq!(run(&mut server, "clone 0 -59 0 1 -59 1 0 -50 0 masked"), ["No blocks were cloned"]);
    assert_eq!(run(&mut server, "clone 0 -60 0 1 -60 1 5 -60 5 filtered minecraft:oak_stairs"), ["Successfully cloned 1 block(s)"]);
    assert!(block(&mut server, 6, -60, 6).is_air());
    assert_eq!(run(&mut server, "clone 0 -60 0 1 -60 1 1 -60 1 replace move"), ["Successfully cloned 4 block(s)"]);
    assert!(block(&mut server, 0, -60, 0).is_air() && block(&mut server, 1, -60, 1).is_block(blocks::OAK_STAIRS));
    assert_eq!(run(&mut server, "clone from minecraft:the_nether 0 -60 0 1 -60 1 0 -50 0"), ["Unknown dimension 'minecraft:the_nether'"]);
    assert_eq!(run(&mut server, "clone from minecraft:overworld 0 -55 0 1 -55 1 to minecraft:overworld 0 -50 0 strict"), ["Successfully cloned 4 block(s)"]);
    assert_eq!(run(&mut server, "clone 0 -60 0 1 -60 1 100 -60 0"), ["That position is not loaded"]);
    assert_eq!(run(&mut server, "clone -16 -60 -16 24 -20 24 0 0 0 replace force"), ["Too many blocks in the specified area (maximum 32768, but specified 68921)"]);
}

#[test]
fn clone_copies_scheduled_ticks() {
    let mut server = server();
    run(&mut server, "setblock 0 -55 0 sand strict");
    server.level.schedule_tick(BlockPos::new(0, -55, 0), blocks::SAND, 2);
    run(&mut server, "clone 0 -55 0 0 -55 0 3 -55 3 strict");
    assert!(server.level.has_scheduled_tick(BlockPos::new(3, -55, 3), blocks::SAND));
}

#[test]
fn give() {
    let mut server = server();
    assert_eq!(run(&mut server, "give Alice stone"), ["No player was found"]);
    let alice = join(&mut server, "Alice");
    assert_eq!(run(&mut server, "give Alice stone 70"), ["Gave 70 [Stone] to Alice"]);
    let inventory = &server.level.player(alice).inventory;
    assert_eq!((inventory.slots[36].clone(), inventory.slots[37].clone()), (Some(ItemStack::new(items::STONE, 64)), Some(ItemStack::new(items::STONE, 6))));
    let fake = items_in_level(&server);
    assert_eq!((fake.len(), fake[0].pickup_delay, fake[0].age, fake[0].item.count), (2, 32767, 5999, 1));
    assert_eq!(run(&mut server, "give Alice stone 6401"), ["Can't give more than 6400 of [Stone]"]);
    assert_eq!(run(&mut server, "give Alice diamond_sword[damage=5]"), ["Gave 1 [Diamond Sword] to Alice"]);
    assert_eq!(server.level.player(alice).inventory.slots[38].as_ref().map(|s| s.damage_value()), Some(5));
    assert_eq!(run(&mut server, "give Alice stone[custom_name=\"Bob\"]"), ["Gave 1 [Bob] to Alice"]);
    assert_eq!(run(&mut server, "give Alice potion"), ["Gave 1 [Uncraftable Potion] to Alice"]);
    assert_eq!(run(&mut server, "give Alice diamond_sword[max_stack_size=2]"), ["Malformed item: 'Item cannot be both damageable and stackable'"]);
    assert_eq!(run(&mut server, "give Alice elytra[!max_stack_size] 101"), ["Can't give more than 100 of [Elytra]"]);
}

#[test]
fn give_drops_what_does_not_fit() {
    let mut server = server();
    let alice = join(&mut server, "Alice");
    let uuid = server.level.player(alice).profile.uuid;
    for slot in 9..=44 {
        server.level.player_mut(alice).inventory.slots[slot] = Some(ItemStack::new(items::DIRT, 64));
    }
    assert_eq!(run(&mut server, "give Alice stone 3"), ["Gave 3 [Stone] to Alice"]);
    let drops = items_in_level(&server);
    assert_eq!((drops.len(), drops[0].item.count, drops[0].pickup_delay, drops[0].target), (1, 3, 0, Some(uuid)));
    server.level.entities.clear();
    server.level.player_mut(alice).game_mode.game_type = GameType::Creative;
    server.level.player_mut(alice).abilities.instabuild = true;
    assert_eq!(run(&mut server, "give Alice stone 3"), ["Gave 3 [Stone] to Alice"]);
    let fake = items_in_level(&server);
    assert_eq!((fake.len(), fake[0].pickup_delay), (1, 32767));
}

#[test]
fn clear() {
    let mut server = server();
    assert_eq!(run(&mut server, "clear"), ["A player is required to run this command here"]);
    let alice = join(&mut server, "Alice");
    assert_eq!(run(&mut server, "clear Alice"), ["No items were found on player Alice"]);
    run(&mut server, "give Alice stone 5");
    run(&mut server, "give Alice diamond_sword[damage=5]");
    run(&mut server, "give Alice iron_sword");
    assert_eq!(run(&mut server, "clear Alice stone 0"), ["Found 5 matching item(s) on player Alice"]);
    assert_eq!(run(&mut server, "clear Alice stone 2"), ["Removed 2 item(s) from player Alice"]);
    assert_eq!(server.level.player(alice).inventory.slots[36], Some(ItemStack::new(items::STONE, 3)));
    assert_eq!(run(&mut server, "clear Alice *[damage~{damage:5}]"), ["Removed 1 item(s) from player Alice"]);
    assert_eq!(run(&mut server, "clear Alice *[damage=0] 0"), ["Found 1 matching item(s) on player Alice"]);
    assert_eq!(run(&mut server, "clear Alice *[!damage] 0"), ["Found 3 matching item(s) on player Alice"]);
    assert_eq!(run(&mut server, "clear Alice *[count=3] 0"), ["Found 3 matching item(s) on player Alice"]);
    assert_eq!(run(&mut server, "clear Alice #minecraft:swords 0"), ["Found 1 matching item(s) on player Alice"]);
    server.level.player_mut(alice).carried = Some(ItemStack::new(items::DIRT, 4));
    server.level.player_mut(alice).inventory.slots[1] = Some(ItemStack::new(items::DIRT, 1));
    assert_eq!(run(&mut server, "clear Alice"), ["Removed 9 item(s) from player Alice"]);
    let player = server.level.player(alice);
    assert!(player.carried.is_none() && player.inventory.slots.iter().all(Option::is_none));
}

#[test]
fn clear_many_players() {
    let mut server = server();
    join(&mut server, "Alice");
    join(&mut server, "Bob");
    assert_eq!(run(&mut server, "clear @a"), ["No items were found on 2 players"]);
    run(&mut server, "give Bob stone");
    assert_eq!(run(&mut server, "clear @a"), ["Removed 1 item(s) from player Bob"]);
    run(&mut server, "give @a stone 2");
    assert_eq!(run(&mut server, "clear @a stone 0"), ["Found 4 matching item(s) on 2 players"]);
}
