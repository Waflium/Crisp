use crate::commands::completion_suggestions;
use crate::commands::source::CommandSourceStack;
use crate::server::Server;

fn server() -> Server {
    Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap()
}

fn suggest(server: &Server, command: &str) -> (usize, usize, Vec<String>) {
    let source = CommandSourceStack::server(server);
    let suggestions = completion_suggestions(server, &source, command).expect("a suggestion context");
    (suggestions.range.start, suggestions.range.end, suggestions.list.into_iter().map(|s| s.text).collect())
}

#[test]
fn literals_complete_from_the_tree() {
    let server = server();
    assert_eq!(suggest(&server, "/time "), (6, 6, ["add", "of", "pause", "query", "rate", "resume", "set"].map(String::from).to_vec()));
    assert_eq!(suggest(&server, "/time re"), (6, 8, vec!["resume".to_string()]));
}

#[test]
fn time_markers_and_units_come_from_the_server() {
    let server = server();
    let (start, end, markers) = suggest(&server, "/time set ");
    assert_eq!((start, end), (10, 10));
    assert!(markers.contains(&"minecraft:day".to_string()) && markers.contains(&"minecraft:night".to_string()), "{markers:?}");
    assert!(markers.is_sorted_by_key(|m| m.to_lowercase()));
    assert_eq!(suggest(&server, "/time set d"), (10, 11, vec!["minecraft:day".to_string()]));
    assert_eq!(suggest(&server, "/time set 1"), (11, 11, ["d", "s", "t"].map(String::from).to_vec()));
}

#[test]
fn tick_rate_suggests_the_default() {
    let server = server();
    assert_eq!(suggest(&server, "/tick rate "), (11, 11, vec!["20.0".to_string()]));
}

#[test]
fn score_holders_suggest_selectors() {
    let server = server();
    let selectors = ["@a", "@e", "@n", "@p", "@r", "@s"].map(String::from).to_vec();
    assert_eq!(suggest(&server, "/scoreboard players set "), (24, 24, selectors.clone()));
    assert_eq!(suggest(&server, "/scoreboard players set @"), (24, 25, selectors));
    assert_eq!(suggest(&server, "/scoreboard players set @e"), (26, 26, vec!["[".to_string()]));
    let (_, _, options) = suggest(&server, "/scoreboard players set @e[");
    assert!(!options.contains(&"]".to_string()) && options.contains(&"type=".to_string()) && options.contains(&"limit=".to_string()), "{options:?}");
    assert_eq!(suggest(&server, "/scoreboard players set @e[sort="), (32, 32, ["arbitrary", "furthest", "nearest", "random"].map(String::from).to_vec()));
}

#[test]
fn coordinates_suggest_relative_positions() {
    let server = server();
    assert_eq!(suggest(&server, "/setblock "), (10, 10, ["~", "~ ~", "~ ~ ~"].map(String::from).to_vec()));
    assert_eq!(suggest(&server, "/setblock 1 "), (10, 12, ["1 ~", "1 ~ ~"].map(String::from).to_vec()));
}
