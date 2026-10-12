use std::fmt::Write as _;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use crisp_data::packets::{configuration, handshake, login, play};
use crisp_data::{PROTOCOL_VERSION, packet_name};
use crisp_protocol::{Codec, Reader, Writer};
use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

struct Options {
    addr: String,
    duration: Duration,
    out: String,
    view_distance: u8,
    name: String,
    wait: Duration,
    walk: bool,
    say: Option<String>,
    dig: Option<u32>,
    script: Vec<String>,
}

fn options() -> anyhow::Result<Options> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: crisp-probe <host:port> <seconds> <out.jsonl> [--view N] [--name NAME] [--wait MS] [--walk] [--say TEXT] [--dig TICKS] [--script FILE]";
    let mut o = Options {
        addr: args.next().context(usage)?,
        duration: Duration::from_secs(args.next().context(usage)?.parse()?),
        out: args.next().context(usage)?,
        view_distance: 10,
        name: "CrispProbe".into(),
        wait: Duration::ZERO,
        walk: false,
        say: None,
        dig: None,
        script: Vec::new(),
    };
    while let Some(flag) = args.next() {
        let mut value = || args.next().with_context(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--view" => o.view_distance = value()?.parse()?,
            "--name" => o.name = value()?,
            "--wait" => o.wait = Duration::from_millis(value()?.parse()?),
            "--walk" => o.walk = true,
            "--say" => o.say = Some(value()?),
            "--dig" => o.dig = Some(value()?.parse()?),
            "--script" => o.script = std::fs::read_to_string(value()?)?.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect(),
            _ => bail!("unknown option {flag}\n{usage}"),
        }
    }
    Ok(o)
}

struct Recorder {
    start: Instant,
    out: BufWriter<tokio::fs::File>,
    seen: Vec<(String, usize)>,
}

impl Recorder {
    async fn record(&mut self, state: &str, at: Instant, bytes: &[u8]) -> anyhow::Result<(i32, Vec<u8>)> {
        let mut r = Reader::new(bytes);
        let id = r.varint()?;
        let data = bytes[bytes.len() - r.remaining()..].to_vec();
        let name = packet_name(state, "clientbound", id).unwrap_or("unknown");
        let mut hex = String::with_capacity(data.len() * 2);
        data.iter().for_each(|b| write!(hex, "{b:02x}").unwrap());
        let line = serde_json::json!({
            "t_ms": at.duration_since(self.start).as_millis() as u64,
            "state": state,
            "id": id,
            "name": name,
            "len": data.len(),
            "data": hex,
        });
        self.out.write_all(format!("{line}\n").as_bytes()).await?;
        match self.seen.last_mut() {
            Some((last, count)) if last == name => *count += 1,
            _ => self.seen.push((name.to_owned(), 1)),
        }
        Ok((id, data))
    }
}

async fn send<W: AsyncWrite + Unpin>(w: &mut W, codec: Codec, id: i32, body: impl FnOnce(&mut Writer)) -> anyhow::Result<()> {
    let mut p = Writer::new();
    p.varint(id);
    body(&mut p);
    w.write_all(&codec.encode(p.as_slice())).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let o = options()?;
    tokio::time::sleep(o.wait).await;
    let (host, port) = o.addr.rsplit_once(':').context("address must be host:port")?;
    let mut rec = Recorder {
        start: Instant::now(),
        out: BufWriter::new(tokio::fs::File::create(&o.out).await?),
        seen: Vec::new(),
    };
    let mut stream = TcpStream::connect(&o.addr).await?;
    stream.set_nodelay(true)?;
    let mut codec = Codec::default();

    send(&mut stream, codec, handshake::serverbound::INTENTION, |w| {
        w.varint(PROTOCOL_VERSION);
        w.string(host);
        w.u16(port.parse().unwrap());
        w.varint(2);
    })
    .await?;
    send(&mut stream, codec, login::serverbound::HELLO, |w| {
        w.string(&o.name);
        w.uuid(0);
    })
    .await?;
    loop {
        let bytes = codec.read(&mut stream).await?;
        let (id, data) = rec.record("login", Instant::now(), &bytes).await?;
        match id {
            login::clientbound::LOGIN_COMPRESSION => codec.threshold = Some(Reader::new(&data).varint()? as usize),
            login::clientbound::LOGIN_FINISHED => break,
            login::clientbound::LOGIN_DISCONNECT => bail!("disconnected during login: {}", Reader::new(&data).string(262144)?),
            _ => {}
        }
    }
    send(&mut stream, codec, login::serverbound::LOGIN_ACKNOWLEDGED, |_| {}).await?;
    send(&mut stream, codec, configuration::serverbound::CLIENT_INFORMATION, |w| {
        w.string("en_us");
        w.u8(o.view_distance);
        w.varint(0);
        w.bool(true);
        w.u8(0x7F);
        w.varint(1);
        w.bool(false);
        w.bool(true);
        w.varint(0);
    })
    .await?;
    loop {
        let bytes = codec.read(&mut stream).await?;
        let (id, data) = rec.record("configuration", Instant::now(), &bytes).await?;
        match id {
            configuration::clientbound::SELECT_KNOWN_PACKS => {
                send(&mut stream, codec, configuration::serverbound::SELECT_KNOWN_PACKS, |w| w.bytes(&data)).await?
            }
            configuration::clientbound::KEEP_ALIVE => {
                send(&mut stream, codec, configuration::serverbound::KEEP_ALIVE, |w| w.bytes(&data)).await?
            }
            configuration::clientbound::PING => send(&mut stream, codec, configuration::serverbound::PONG, |w| w.bytes(&data)).await?,
            configuration::clientbound::FINISH_CONFIGURATION => break,
            configuration::clientbound::DISCONNECT => bail!("disconnected during configuration"),
            _ => {}
        }
    }
    send(&mut stream, codec, configuration::serverbound::FINISH_CONFIGURATION, |_| {}).await?;

    let (mut rd, mut wr) = stream.into_split();
    let (tx, mut rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Ok(bytes) = codec.read(&mut rd).await {
            if tx.send((Instant::now(), bytes)).is_err() {
                break;
            }
        }
    });

    let deadline = tokio::time::Instant::now() + o.duration;
    let mut ticker = tokio::time::interval(Duration::from_millis(50));
    let mut position: Option<(f64, f64, f64)> = None;
    let mut ticks_loaded = 0u32;
    let mut script = o.script.iter();
    let mut script_wait = 0u32;
    let mut sequence = 100;
    let mut open_container = 0;
    let mut on_ground = 1u8;
    let mut last_entity = 0;
    loop {
        tokio::select! {
            packet = rx.recv() => {
                let Some((at, bytes)) = packet else { break };
                let (id, data) = rec.record("play", at, &bytes).await?;
                match id {
                    play::clientbound::KEEP_ALIVE => send(&mut wr, codec, play::serverbound::KEEP_ALIVE, |w| w.bytes(&data)).await?,
                    play::clientbound::PING => send(&mut wr, codec, play::serverbound::PONG, |w| w.bytes(&data)).await?,
                    play::clientbound::CHUNK_BATCH_FINISHED => {
                        send(&mut wr, codec, play::serverbound::CHUNK_BATCH_RECEIVED, |w| w.f32(64.0)).await?
                    }
                    play::clientbound::PLAYER_POSITION => {
                        let mut r = Reader::new(&data);
                        let teleport = r.varint()?;
                        let pos = (r.f64()?, r.f64()?, r.f64()?);
                        if position.is_none() {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                        send(&mut wr, codec, play::serverbound::ACCEPT_TELEPORTATION, |w| {
                            w.varint(teleport);
                            w.f64(pos.0);
                            w.f64(pos.1);
                            w.f64(pos.2);
                            w.f32(0.0);
                            w.f32(0.0);
                        })
                        .await?;
                        if position.is_none() {
                            send(&mut wr, codec, play::serverbound::PLAYER_LOADED, |_| {}).await?;
                        }
                        position = Some(pos);
                    }
                    play::clientbound::OPEN_SCREEN => open_container = Reader::new(&data).varint()?,
                    play::clientbound::ADD_ENTITY => last_entity = Reader::new(&data).varint()?,
                    play::clientbound::DISCONNECT => break,
                    _ => {}
                }
            }
            _ = ticker.tick() => {
                let Some((x, y, z)) = position.as_mut() else { continue };
                ticks_loaded += 1;
                if o.walk {
                    *x += 0.2;
                    let (x, y, z) = (*x, *y, *z);
                    send(&mut wr, codec, play::serverbound::MOVE_PLAYER_POS, |w| {
                        w.f64(x);
                        w.f64(y);
                        w.f64(z);
                        w.u8(1);
                    })
                    .await?;
                }
                if ticks_loaded == 20 && let Some(command) = o.say.as_ref().and_then(|m| m.strip_prefix('/')) {
                    send(&mut wr, codec, play::serverbound::CHAT_COMMAND, |w| w.string(command)).await?;
                } else if ticks_loaded == 20 && let Some(message) = &o.say {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
                    send(&mut wr, codec, play::serverbound::CHAT, |w| {
                        w.string(message);
                        w.i64(now);
                        w.i64(0);
                        w.bool(false);
                        w.varint(0);
                        w.bytes(&[0, 0, 0]);
                        w.u8(1);
                    })
                    .await?;
                }
                if ticks_loaded >= 20 && script_wait > 0 {
                    script_wait -= 1;
                } else if ticks_loaded >= 20 && let Some(line) = script.next() {
                    let words: Vec<&str> = line.split_whitespace().collect();
                    match words.as_slice() {
                        ["wait", ticks] => script_wait = ticks.parse()?,
                        ["respawn"] => send(&mut wr, codec, play::serverbound::CLIENT_COMMAND, |w| w.varint(0)).await?,
                        ["stats"] => send(&mut wr, codec, play::serverbound::CLIENT_COMMAND, |w| w.varint(1)).await?,
                        ["input", flags] => {
                            let flags: u8 = flags.parse()?;
                            send(&mut wr, codec, play::serverbound::PLAYER_INPUT, |w| w.u8(flags)).await?;
                        }
                        ["action", status, x, y, z, face] => {
                            let (status, x, y, z, face): (i32, i32, i32, i32, u8) = (status.parse()?, x.parse()?, y.parse()?, z.parse()?, face.parse()?);
                            sequence += 1;
                            send(&mut wr, codec, play::serverbound::PLAYER_ACTION, |w| {
                                w.varint(status);
                                w.position(x, y, z);
                                w.u8(face);
                                w.varint(sequence);
                            })
                            .await?;
                        }
                        ["move_air", nx, ny, nz, y_rot] => {
                            let (nx, ny, nz, y_rot): (f64, f64, f64, f32) = (nx.parse()?, ny.parse()?, nz.parse()?, y_rot.parse()?);
                            (*x, *y, *z) = (nx, ny, nz);
                            on_ground = 0;
                            send(&mut wr, codec, play::serverbound::MOVE_PLAYER_POS_ROT, |w| {
                                w.f64(nx);
                                w.f64(ny);
                                w.f64(nz);
                                w.f32(y_rot);
                                w.f32(0.0);
                                w.u8(0);
                            })
                            .await?;
                        }
                        ["sign", x, y, z, slot, rest @ ..] => {
                            let (x, y, z, slot): (i32, i32, i32, i32) = (x.parse()?, y.parse()?, z.parse()?, slot.parse()?);
                            let text = rest.join(" ");
                            let mut lines: Vec<&str> = text.split('|').collect();
                            lines.resize(4, "");
                            send(&mut wr, codec, play::serverbound::SIGN_UPDATE, |w| {
                                w.position(x, y, z);
                                for line in &lines[..4] {
                                    w.string(line);
                                }
                                w.varint(slot);
                            })
                            .await?;
                        }
                        ["button", button] => {
                            let button: i32 = button.parse()?;
                            send(&mut wr, codec, play::serverbound::CONTAINER_BUTTON_CLICK, |w| {
                                w.varint(open_container);
                                w.varint(button);
                            })
                            .await?;
                        }
                        ["beacon", primary, secondary] => {
                            let (primary, secondary): (i32, i32) = (primary.parse()?, secondary.parse()?);
                            send(&mut wr, codec, play::serverbound::SET_BEACON, |w| {
                                for effect in [primary, secondary] {
                                    w.bool(effect >= 0);
                                    if effect >= 0 {
                                        w.varint(effect);
                                    }
                                }
                            })
                            .await?;
                        }
                        ["click", slot, button, mode] => {
                            let (slot, button, mode): (i16, u8, i32) = (slot.parse()?, button.parse()?, mode.parse()?);
                            send(&mut wr, codec, play::serverbound::CONTAINER_CLICK, |w| {
                                w.varint(open_container);
                                w.varint(0);
                                w.i16(slot);
                                w.u8(button);
                                w.varint(mode);
                                w.varint(0);
                                w.bool(false);
                            })
                            .await?;
                        }
                        ["place_recipe", recipe, use_max_items] => {
                            let (recipe, use_max_items): (i32, bool) = (recipe.parse()?, use_max_items.parse()?);
                            send(&mut wr, codec, play::serverbound::PLACE_RECIPE, |w| {
                                w.varint(open_container);
                                w.varint(recipe);
                                w.bool(use_max_items);
                            })
                            .await?;
                        }
                        ["book_settings", book_type, is_open, is_filtering] => {
                            let (book_type, is_open, is_filtering): (i32, bool, bool) = (book_type.parse()?, is_open.parse()?, is_filtering.parse()?);
                            send(&mut wr, codec, play::serverbound::RECIPE_BOOK_CHANGE_SETTINGS, |w| {
                                w.varint(book_type);
                                w.bool(is_open);
                                w.bool(is_filtering);
                            })
                            .await?;
                        }
                        ["rename", rest @ ..] => {
                            let name = rest.join(" ");
                            send(&mut wr, codec, play::serverbound::RENAME_ITEM, |w| w.string(&name)).await?;
                        }
                        ["slot_state", slot_id, new_state] => {
                            let (slot_id, new_state): (i32, bool) = (slot_id.parse()?, new_state.parse()?);
                            send(&mut wr, codec, play::serverbound::CONTAINER_SLOT_STATE_CHANGED, |w| {
                                w.varint(slot_id);
                                w.varint(open_container);
                                w.bool(new_state);
                            })
                            .await?;
                        }
                        ["advancement_tab", tab] => {
                            send(&mut wr, codec, play::serverbound::SEEN_ADVANCEMENTS, |w| {
                                w.varint(0);
                                w.string(tab);
                            })
                            .await?;
                        }
                        ["advancement_close"] => send(&mut wr, codec, play::serverbound::SEEN_ADVANCEMENTS, |w| w.varint(1)).await?,
                        ["interact_last", hand] => {
                            let hand: i32 = hand.parse()?;
                            send(&mut wr, codec, play::serverbound::INTERACT, |w| {
                                w.varint(last_entity);
                                w.varint(hand);
                                w.lp_vec3(0.0, 0.2, 0.0);
                                w.bool(false);
                            })
                            .await?;
                        }
                        ["seen_recipe", recipe] => {
                            let recipe: i32 = recipe.parse()?;
                            send(&mut wr, codec, play::serverbound::RECIPE_BOOK_SEEN_RECIPE, |w| w.varint(recipe)).await?;
                        }
                        ["close"] => {
                            send(&mut wr, codec, play::serverbound::CONTAINER_CLOSE, |w| w.varint(open_container)).await?;
                            open_container = 0;
                        }
                        ["quick", slot] => {
                            let slot: i16 = slot.parse()?;
                            send(&mut wr, codec, play::serverbound::CONTAINER_CLICK, |w| {
                                w.varint(open_container);
                                w.varint(0);
                                w.i16(slot);
                                w.u8(0);
                                w.varint(1);
                                w.varint(0);
                                w.bool(false);
                            })
                            .await?;
                        }
                        ["use_item", y_rot, x_rot] => {
                            let (y_rot, x_rot): (f32, f32) = (y_rot.parse()?, x_rot.parse()?);
                            sequence += 1;
                            send(&mut wr, codec, play::serverbound::USE_ITEM, |w| {
                                w.varint(0);
                                w.varint(sequence);
                                w.f32(y_rot);
                                w.f32(x_rot);
                            })
                            .await?;
                        }
                        ["use_item", hand] => {
                            let hand: i32 = hand.parse()?;
                            sequence += 1;
                            send(&mut wr, codec, play::serverbound::USE_ITEM, |w| {
                                w.varint(hand);
                                w.varint(sequence);
                                w.f32(0.0);
                                w.f32(0.0);
                            })
                            .await?;
                        }
                        ["hotbar", slot] => {
                            let slot: i16 = slot.parse()?;
                            send(&mut wr, codec, play::serverbound::SET_CARRIED_ITEM, |w| w.i16(slot)).await?;
                        }
                        ["move", nx, ny, nz, y_rot] => {
                            let (nx, ny, nz, y_rot): (f64, f64, f64, f32) = (nx.parse()?, ny.parse()?, nz.parse()?, y_rot.parse()?);
                            (*x, *y, *z) = (nx, ny, nz);
                            on_ground = 1;
                            send(&mut wr, codec, play::serverbound::MOVE_PLAYER_POS_ROT, |w| {
                                w.f64(nx);
                                w.f64(ny);
                                w.f64(nz);
                                w.f32(y_rot);
                                w.f32(0.0);
                                w.u8(1);
                            })
                            .await?;
                        }
                        ["use", x, y, z, face] => {
                            let (x, y, z, face): (i32, i32, i32, i32) = (x.parse()?, y.parse()?, z.parse()?, face.parse()?);
                            sequence += 1;
                            send(&mut wr, codec, play::serverbound::USE_ITEM_ON, |w| {
                                w.varint(0);
                                w.position(x, y, z);
                                w.varint(face);
                                w.f32(0.5);
                                w.f32(0.5);
                                w.f32(0.5);
                                w.bool(false);
                                w.bool(false);
                                w.varint(sequence);
                            })
                            .await?;
                        }
                        _ => {
                            let command = line.strip_prefix('/').unwrap_or(line);
                            send(&mut wr, codec, play::serverbound::CHAT_COMMAND, |w| w.string(command)).await?;
                        }
                    }
                }
                if o.dig.is_some() {
                    send(&mut wr, codec, play::serverbound::MOVE_PLAYER_ROT, |w| {
                        w.f32(0.0);
                        w.f32(90.0);
                        w.u8(1);
                    })
                    .await?;
                } else if !o.walk {
                    send(&mut wr, codec, play::serverbound::MOVE_PLAYER_STATUS_ONLY, |w| w.u8(on_ground)).await?;
                }
                if let Some(dig_ticks) = o.dig {
                    let below = (x.floor() as i32, y.floor() as i32 - 1, z.floor() as i32);
                    let action = if ticks_loaded == 20 { Some((0, 1)) } else if ticks_loaded == 20 + dig_ticks { Some((3, 2)) } else { None };
                    if (20..20 + dig_ticks).contains(&ticks_loaded) {
                        send(&mut wr, codec, play::serverbound::PUNCH, |_| {}).await?;
                    }
                    if let Some((status, sequence)) = action {
                        send(&mut wr, codec, play::serverbound::PLAYER_ACTION, |w| {
                            w.varint(status);
                            w.position(below.0, below.1, below.2);
                            w.u8(1);
                            w.varint(sequence);
                        })
                        .await?;
                    }
                }
                send(&mut wr, codec, play::serverbound::CLIENT_TICK_END, |_| {}).await?;
            }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }
    rec.out.flush().await?;
    for (name, count) in &rec.seen {
        println!("{count:>5} x {name}");
    }
    Ok(())
}
