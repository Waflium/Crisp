use chrono::{DateTime, Local};
use crisp_data::{BUILD_TIME, DATA_PACK_VERSION, PROTOCOL_VERSION, RESOURCE_PACK_VERSION, VERSION_ID, VERSION_NAME, VERSION_SERIES, VERSION_STABLE, WORLD_VERSION};

use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;

pub fn version(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    source.send_system_message(server, &Component::translatable("commands.version.header"));
    for line in dump_version() {
        source.send_system_message(server, &line);
    }
    Ok(1)
}

pub fn dump_version() -> Vec<Component> {
    let line = |key: &str, args: Vec<Arg>| Component::translatable_with(key, args);
    let pack = |(major, minor): (i32, i32)| format!("{major}.{minor}");
    vec![
        line("commands.version.id", vec![VERSION_ID.into()]),
        line("commands.version.name", vec![VERSION_NAME.into()]),
        line("commands.version.data", vec![WORLD_VERSION.into()]),
        line("commands.version.series", vec![VERSION_SERIES.into()]),
        line("commands.version.protocol", vec![PROTOCOL_VERSION.into(), format!("0x{PROTOCOL_VERSION:x}").into()]),
        line("commands.version.build_time", vec![Component::literal(java_date_string(BUILD_TIME)).into()]),
        line("commands.version.pack.resource", vec![pack(RESOURCE_PACK_VERSION).into()]),
        line("commands.version.pack.data", vec![pack(DATA_PACK_VERSION).into()]),
        Component::translatable(if VERSION_STABLE { "commands.version.stable.yes" } else { "commands.version.stable.no" }),
    ]
}

fn java_date_string(iso: &str) -> String {
    let time = DateTime::parse_from_rfc3339(iso).expect("an ISO build time").with_timezone(&Local);
    let offset = time.offset().local_minus_utc();
    let zone = if offset == 0 {
        "UTC".to_string()
    } else {
        let sign = if offset < 0 { '-' } else { '+' };
        let minutes = offset.abs() / 60;
        format!("GMT{sign}{:02}:{:02}", minutes / 60, minutes % 60)
    };
    format!("{} {zone} {}", time.format("%a %b %d %H:%M:%S"), time.format("%Y"))
}
