use crisp_protocol::nbt::Nbt;
use crisp_protocol::{Reader, Writer};

use super::criteria::{ObjectiveCriteria, RenderType};
use super::display_slot::DisplaySlot;
use super::score_holder::ScoreHolder;
use super::team::{CollisionRule, Visibility};
use super::team_color::TeamColor;
use crate::network::chat::numbers::NumberFormat;
use crate::network::chat::{ChatFormatting, Component, Style};
use crate::server::level::ServerLevel;
use crate::server::level::testing;
use crate::server::server_scoreboard::*;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn body(packet: Vec<u8>) -> String {
    hex(&packet[1..])
}

fn component_hex(component: &Component) -> String {
    let mut w = Writer::new();
    component.write(&mut w);
    hex(w.as_slice())
}

fn set(level: &mut ServerLevel, holder: &str, objective: u32, value: i32) {
    let mut score = level.scoreboard.borrow_mut().get_or_create_player_score(&ScoreHolder::for_name_only(holder), objective, false);
    score.set(level, value).unwrap();
}

fn joined<'a>(names: impl Iterator<Item = &'a str>) -> String {
    names.collect::<Vec<_>>().join(",")
}

#[test]
fn maps_iterate_as_vanilla() {
    let mut level = testing::level();
    let mut objectives = Vec::new();
    for i in 0..40 {
        let name = if i % 2 == 0 { format!("obj{i}") } else { format!("o_{}", i * 7919) };
        objectives.push(level.scoreboard.borrow_mut().add_objective(&name, ObjectiveCriteria::DUMMY, Component::literal(name.clone()), RenderType::Integer, false, None));
    }
    for i in (0..40).step_by(3) {
        level.scoreboard.borrow_mut().remove_objective(objectives[i]);
    }
    let stored = level.scoreboard.borrow().store();
    let streamed = joined(stored.get("Objectives").unwrap().as_list().iter().map(|o| o.get("Name").unwrap().as_str().unwrap()));
    assert_eq!(
        streamed,
        "obj10,o_150461,o_245489,o_87109,obj16,o_102947,o_7919,obj32,o_197975,o_134623,o_39595,o_182137,obj34,obj22,o_55433,obj4,obj28,obj26,obj2,obj8,o_293003,obj20,o_277165,obj14,obj38,o_229651"
    );
    assert_eq!(
        joined(level.scoreboard.borrow().objectives().map(|o| o.name())),
        "o_229651,obj38,obj14,o_277165,obj20,o_293003,obj8,obj2,obj26,obj28,obj4,o_55433,obj22,obj34,o_182137,o_39595,o_134623,o_197975,obj32,o_7919,o_102947,obj16,o_87109,o_245489,o_150461,obj10"
    );

    let first = objectives[1];
    let name = |i: i32| if i % 5 == 0 { format!("#fake{i}") } else { format!("Player{i}") };
    for i in 0..70 {
        set(&mut level, &name(i), first, i);
    }
    for i in 0..70 {
        if i % 7 != 3 {
            level.scoreboard.borrow_mut().reset_all_player_scores(&name(i));
        }
    }
    let mut scoreboard = level.scoreboard.borrow_mut();
    let tracked: Vec<String> = scoreboard.tracked_players().into_iter().map(|h| h.name).collect();
    assert_eq!(tracked.join(","), "Player17,Player59,Player24,Player52,Player38,Player31,#fake45,Player66,Player3,#fake10");
    assert_eq!(joined(scoreboard.list_player_scores(first).iter().map(|e| e.owner)), "#fake10,Player3,Player66,#fake45,Player31,Player38,Player52,Player24,Player59,Player17");

    scoreboard.add_player_team("crowd");
    for i in 0..30 {
        scoreboard.add_player_to_team(&format!("member{}", i * 31), "crowd");
    }
    for i in (0..30).step_by(4) {
        scoreboard.remove_player_from_team(&format!("member{}", i * 31), "crowd");
    }
    assert_eq!(
        joined(scoreboard.get_player_team("crowd").unwrap().players()),
        "member217,member837,member279,member310,member155,member682,member465,member62,member589,member403,member527,member806,member341,member186,member31,member651,member434,member775,member93,member558,member899,member713"
    );
    for name in ["red", "blue", "green", "a", "zz", "Team1"] {
        scoreboard.add_player_team(name);
    }
    assert_eq!(joined(scoreboard.player_teams().map(|t| t.name())), "a,blue,Team1,green,crowd,red,zz");
}

#[test]
fn packets_match_vanilla() {
    let level = testing::level();
    let mut scoreboard = level.scoreboard.borrow_mut();
    let gold_bold = Style { color: Some(ChatFormatting::Gold.color()), bold: Some(true), ..Style::default() };
    let foo = scoreboard.add_objective(
        "foo",
        ObjectiveCriteria::DUMMY,
        Component::literal("Foo").with_color(ChatFormatting::Red),
        RenderType::Hearts,
        false,
        Some(NumberFormat::Styled(gold_bold)),
    );
    let bar = scoreboard.add_objective("bar", ObjectiveCriteria::DEATH_COUNT, Component::literal("bar"), RenderType::Integer, false, None);
    assert_eq!(body(set_objective_packet(scoreboard.objective(foo), METHOD_ADD)), "03666f6f000a080005636f6c6f720003726564080004746578740003466f6f000101010a080005636f6c6f720004676f6c64010004626f6c640100");
    assert_eq!(body(set_objective_packet(scoreboard.objective(foo), METHOD_REMOVE)), "03666f6f01");
    scoreboard.set_number_format(bar, Some(NumberFormat::Fixed(Component::literal("-"))));
    assert_eq!(body(set_objective_packet(scoreboard.objective(bar), METHOD_CHANGE)), "03626172020800036261720001020800012d");
    scoreboard.set_number_format(bar, Some(NumberFormat::Blank));
    assert_eq!(body(set_objective_packet(scoreboard.objective(bar), METHOD_CHANGE)), "0362617202080003626172000100");
    assert_eq!(body(set_display_objective_packet(DisplaySlot::Sidebar, Some(scoreboard.objective(foo)))), "0103666f6f");
    assert_eq!(body(set_display_objective_packet(DisplaySlot::TeamRed, None)), "0f00");
    assert_eq!(body(set_score_packet("alice", "foo", 5, None, None)), "05616c69636503666f6f050000");
    let display = Component::literal("X").with_color(ChatFormatting::Aqua);
    assert_eq!(
        body(set_score_packet("#x", "foo", -300, Some(&display), Some(&NumberFormat::Blank))),
        "02237803666f6fd4fdffff0f010a080005636f6c6f7200046171756108000474657874000158000100"
    );
    assert_eq!(body(reset_score_packet("alice", None)), "05616c69636500");
    assert_eq!(body(reset_score_packet("alice", Some("foo"))), "05616c6963650103666f6f");

    scoreboard.add_player_team("red");
    scoreboard.set_team_display_name("red", Component::literal("Reds"));
    scoreboard.set_color("red", Some(TeamColor::Red));
    scoreboard.set_player_prefix("red", Component::literal("[R] "));
    scoreboard.set_allow_friendly_fire("red", false);
    scoreboard.set_name_tag_visibility("red", Visibility::HideForOtherTeams);
    scoreboard.set_collision_rule("red", CollisionRule::PushOwnTeam);
    scoreboard.add_player_to_team("alice", "red");
    scoreboard.add_player_to_team("bob", "red");
    let red = scoreboard.get_player_team("red").unwrap();
    assert_eq!(body(set_player_team_add_or_modify_packet(red, true)), "0372656400080004526564730800045b525d200800000203010c020203626f6205616c696365");
    assert_eq!(body(set_player_team_add_or_modify_packet(red, false)), "0372656402080004526564730800045b525d200800000203010c02");
    assert_eq!(body(set_player_team_remove_packet(red)), "0372656401");
    assert_eq!(body(set_player_team_player_packet("red", "carol", true)), "037265640301056361726f6c");
    assert_eq!(body(set_player_team_player_packet("red", "bob", false)), "03726564040103626f62");
    assert_eq!(
        component_hex(&red.formatted_display_name()),
        "0a090004776974680a00000001080009696e73657274696f6e0003726564080004746578740004526564730a000b686f7665725f6576656e74080006616374696f6e000973686f775f7465787408000576616c756500037265640000080005636f6c6f7200037265640800097472616e736c6174650014636861742e7371756172655f627261636b65747300"
    );
    assert_eq!(component_hex(&red.formatted_name(Component::literal("alice"))), "0a080005636f6c6f7200037265640900056578747261080000000300045b525d200005616c696365000008000474657874000000");
    assert_eq!(
        component_hex(&scoreboard.objective(foo).formatted_display_name()),
        "0a090004776974680a00000001080005636f6c6f720003726564080004746578740003466f6f0a000b686f7665725f6576656e74080006616374696f6e000973686f775f7465787408000576616c75650003666f6f00000800097472616e736c6174650014636861742e7371756172655f627261636b65747300"
    );
}

const SAVE: &str = concat!(
    "0a09000c506c6179657253636f7265730a000000040800094f626a6563746976650005706c61696e0100064c6f636b6564010800044e616d6500056361726f6c00080009",
    "4f626a6563746976650005706c61696e0100064c6f636b65640103000553636f72650000002a0800044e616d65000723676c6f62616c000800094f626a65637469766500",
    "05706c61696e03000553636f7265fffffff9080007646973706c61790005426f6262790a0006666f726d617408000474797065000f6d696e6563726166743a626c616e6b",
    "000800044e616d650003626f62000800094f626a65637469766500056b696c6c730100064c6f636b65640103000553636f7265000000030800044e616d650005616c6963",
    "65000a000c446973706c6179536c6f74730800077369646562617200056b696c6c73080011736964656261722e7465616d2e676f6c640005706c61696e0800046c697374",
    "000268700009000a4f626a656374697665730a0000000408000c43726974657269614e616d6500066865616c74680a000b446973706c61794e616d65080005636f6c6f72",
    "00037265640800047465787400024850000a0006666f726d617408000474797065000f6d696e6563726166743a666978656408000576616c756500013f0008000a52656e",
    "6465725479706500066865617274730800044e616d65000268700008000c43726974657269614e616d6500077472696767657208000b446973706c61794e616d65000474",
    "7269670a0006666f726d6174080005636f6c6f7200037265640800047479706500106d696e6563726166743a7374796c6564000800044e616d650004747269670008000c",
    "43726974657269614e616d65000e746f74616c4b696c6c436f756e7408000b446973706c61794e616d6500054b696c6c73010013646973706c61795f6175746f5f757064",
    "617465010800044e616d6500056b696c6c730008000b446973706c61794e616d650005706c61696e0800044e616d650005706c61696e000900055465616d730a00000002",
    "08001644656174684d6573736167655669736962696c69747900056e65766572010015536565467269656e646c79496e76697369626c65730008000b446973706c61794e",
    "616d650004626c75650800095465616d436f6c6f720004626c75650800104d656d6265724e616d655375666669780001210800044e616d650004626c7565090007506c61",
    "7965727308000000020003626f620005616c6963650008000b446973706c61794e616d650005656d7074790800044e616d650005656d7074790000",
);

#[test]
fn save_data_matches_vanilla() {
    let mut level = testing::level();
    let (kills, plain, hearts) = {
        let mut scoreboard = level.scoreboard.borrow_mut();
        let kills = scoreboard.add_objective("kills", ObjectiveCriteria::KILL_COUNT_ALL, Component::literal("Kills"), RenderType::Integer, true, None);
        let plain = scoreboard.add_objective("plain", ObjectiveCriteria::DUMMY, Component::literal("plain"), RenderType::Integer, false, None);
        let hearts = scoreboard.add_objective(
            "hp",
            ObjectiveCriteria::HEALTH,
            Component::literal("HP").with_color(ChatFormatting::Red),
            RenderType::Hearts,
            false,
            Some(NumberFormat::Fixed(Component::literal("?"))),
        );
        scoreboard.add_objective("trig", ObjectiveCriteria::TRIGGER, Component::literal("trig"), RenderType::Integer, false, Some(NumberFormat::sidebar_default()));
        (kills, plain, hearts)
    };
    set(&mut level, "alice", kills, 3);
    set(&mut level, "carol", plain, 0);
    let mut bob = level.scoreboard.borrow_mut().get_or_create_player_score(&ScoreHolder::for_name_only("bob"), plain, false);
    bob.set(&mut level, -7).unwrap();
    bob.unlock(&mut level);
    bob.set_display(&mut level, Some(Component::literal("Bobby")));
    bob.number_format_override(&mut level, Some(NumberFormat::Blank));
    set(&mut level, "#global", plain, 42);
    let mut scoreboard = level.scoreboard.borrow_mut();
    scoreboard.set_display_objective(DisplaySlot::Sidebar, Some(kills));
    scoreboard.set_display_objective(DisplaySlot::List, Some(hearts));
    scoreboard.set_display_objective(DisplaySlot::TeamGold, Some(plain));
    scoreboard.add_player_team("blue");
    scoreboard.set_color("blue", Some(TeamColor::Blue));
    scoreboard.set_see_friendly_invisibles("blue", false);
    scoreboard.set_player_suffix("blue", Component::literal("!"));
    scoreboard.set_death_message_visibility("blue", Visibility::Never);
    scoreboard.add_player_to_team("alice", "blue");
    scoreboard.add_player_to_team("bob", "blue");
    scoreboard.add_player_team("empty");
    let mut w = Writer::new();
    scoreboard.store().write(&mut w);
    assert_eq!(hex(w.as_slice()), SAVE);

    let bytes = unhex(SAVE);
    let tag = Nbt::read(&mut Reader::new(&bytes)).unwrap();
    let loaded = testing::level();
    let mut loaded = loaded.scoreboard.borrow_mut();
    loaded.load(&tag);
    let mut w = Writer::new();
    loaded.store().write(&mut w);
    assert_eq!(hex(w.as_slice()), SAVE);
    let bob = loaded.get_objective("plain").map(|o| o.id()).and_then(|o| loaded.get_player_score_info("bob", o)).unwrap();
    assert_eq!((bob.value, bob.locked, bob.display.clone()), (-7, false, Some(Component::literal("Bobby"))));
    assert!(loaded.get_players_team("alice").is_some_and(|t| t.name() == "blue"));
}

#[test]
fn criteria_by_name() {
    assert_eq!(ObjectiveCriteria::by_name("health").unwrap(), Some(ObjectiveCriteria::HEALTH));
    assert_eq!(ObjectiveCriteria::by_name("teamkill.red").unwrap(), Some(ObjectiveCriteria::team_kill(TeamColor::Red)));
    assert_eq!(ObjectiveCriteria::by_name("used:stone").unwrap().unwrap().name(), "minecraft.used:minecraft.stone");
    assert_eq!(ObjectiveCriteria::by_name("minecraft.custom:minecraft.jump").unwrap().unwrap().name(), "minecraft.custom:minecraft.jump");
    assert_eq!(ObjectiveCriteria::by_name("minecraft.used:nope").unwrap(), None);
    assert_eq!(ObjectiveCriteria::by_name("nope").unwrap(), None);
    assert_eq!(ObjectiveCriteria::by_name("Foo:bar").unwrap_err(), "Non [a-z0-9/._-] character in path of location: minecraft:Foo");
}
