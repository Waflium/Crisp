# Crisp

A Minecraft Java Edition 26.3 server written in Rust.

Crisp aims to behave exactly like the vanilla dedicated server: same gameplay, same console output, same files on disk, same network behaviour. Only the language and the speed differ.

It is far from complete. World generation, players, saving, commands, RCON and Query work; mobs, structures, the Nether and the End do not exist yet.

## Building

Crisp builds its game data from the official server jar, which is downloaded locally and never committed:

```
cargo xtask vanilla
cargo build --release
```

Run `target/release/crisp` in a directory with an accepted `eula.txt`, just like the vanilla server.

## Testing

```
cargo test --release
```
