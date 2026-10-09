use crisp_data::VERSION_NAME;

use super::connection::{PacketReader, PacketWriter};
use super::server_connection_listener::ListenerContext;

const CUSTOM_PAYLOAD_PACKET_ID: u8 = 250;
const CUSTOM_PAYLOAD_PACKET_PING_CHANNEL: &str = "MC|PingHost";
const GET_INFO_PACKET_ID: u8 = 254;
const GET_INFO_PACKET_VERSION_1: u8 = 1;
const DISCONNECT_PACKET_ID: u8 = 255;
const FAKE_PROTOCOL_VERSION: i32 = 127;

pub async fn handle(r: &mut PacketReader, w: &mut PacketWriter, context: &ListenerContext) -> std::io::Result<bool> {
    let first = r.read().await?;
    let Some(body) = respond(first, context) else { return Ok(false) };
    w.send_raw_and_close(&create_legacy_disconnect_packet(&body)).await?;
    Ok(true)
}

fn respond(input: &[u8], context: &ListenerContext) -> Option<String> {
    let mut buf = ByteBuf { data: input, pos: 0 };
    if buf.read_unsigned_byte()? != GET_INFO_PACKET_ID {
        return None;
    }
    if buf.readable_bytes() == 0 {
        return Some(create_version_0_response(context));
    }
    if buf.read_unsigned_byte()? != GET_INFO_PACKET_VERSION_1 {
        return None;
    }
    if buf.readable_bytes() > 0 && !read_custom_payload_packet(&mut buf)? {
        return None;
    }
    Some(create_version_1_response(context))
}

fn read_custom_payload_packet(buf: &mut ByteBuf) -> Option<bool> {
    if buf.read_unsigned_byte()? != CUSTOM_PAYLOAD_PACKET_ID {
        return Some(false);
    }
    if read_legacy_string(buf)? != CUSTOM_PAYLOAD_PACKET_PING_CHANNEL {
        return Some(false);
    }
    let payload_size = buf.read_unsigned_short()?;
    if buf.readable_bytes() != payload_size as usize {
        return Some(false);
    }
    if buf.read_unsigned_byte()? < 73 {
        return Some(false);
    }
    read_legacy_string(buf)?;
    let port = buf.read_int()?;
    Some(port <= 65535)
}

fn player_count(context: &ListenerContext) -> usize {
    context.player_names_array.read().unwrap().len()
}

fn create_version_0_response(context: &ListenerContext) -> String {
    format!("{}\u{a7}{}\u{a7}{}", context.config.motd, player_count(context), context.config.max_players)
}

fn create_version_1_response(context: &ListenerContext) -> String {
    format!(
        "\u{a7}1\0{FAKE_PROTOCOL_VERSION}\0{VERSION_NAME}\0{}\0{}\0{}",
        context.config.motd,
        player_count(context),
        context.config.max_players
    )
}

fn create_legacy_disconnect_packet(reason: &str) -> Vec<u8> {
    let units: Vec<u16> = reason.encode_utf16().collect();
    let mut out = Vec::with_capacity(3 + units.len() * 2);
    out.push(DISCONNECT_PACKET_ID);
    out.extend((units.len() as u16).to_be_bytes());
    units.iter().for_each(|u| out.extend(u.to_be_bytes()));
    out
}

fn read_legacy_string(buf: &mut ByteBuf) -> Option<String> {
    let char_count = buf.read_short()?;
    let byte_count = usize::try_from(char_count as i32 * 2).ok()?;
    let bytes = buf.data.get(buf.pos..buf.pos.checked_add(byte_count)?)?;
    let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
    buf.pos += byte_count;
    Some(String::from_utf16_lossy(&units))
}

struct ByteBuf<'a> {
    data: &'a [u8],
    pos: usize,
}

impl ByteBuf<'_> {
    fn readable_bytes(&self) -> usize {
        self.data.len() - self.pos
    }

    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let bytes = self.data.get(self.pos..self.pos + N)?.try_into().ok()?;
        self.pos += N;
        Some(bytes)
    }

    fn read_unsigned_byte(&mut self) -> Option<u8> {
        self.take::<1>().map(|[b]| b)
    }

    fn read_short(&mut self) -> Option<i16> {
        self.take().map(i16::from_be_bytes)
    }

    fn read_unsigned_short(&mut self) -> Option<u16> {
        self.take().map(u16::from_be_bytes)
    }

    fn read_int(&mut self) -> Option<i32> {
        self.take().map(i32::from_be_bytes)
    }
}
