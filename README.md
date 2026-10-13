<p align="center">
  <img src=".github/logo.png" alt="Crisp" width="280">
</p>

<p align="center">
  A Minecraft Java Edition 26.3 server written in Rust.
</p>

---

Crisp aims to behave exactly like the vanilla dedicated server: the same gameplay, console output, files on disk and network behaviour. Only the language and the speed differ.

## Status

Crisp is a work in progress.

- **Working:** overworld generation and structures, players, saving, commands, data packs, RCON and Query.
- **Not yet:** most mobs, the Nether and the End.

## Building

Crisp builds its game data from the official server jar, which is downloaded locally and never committed:

```sh
cargo xtask vanilla
cargo build --release
```

Run `target/release/crisp` in a directory with an accepted `eula.txt`, just like the vanilla server.

## Testing

```sh
cargo test --release
```

## License

[PolyForm Noncommercial License 1.0.0](LICENSE). You may use, change and share Crisp for any noncommercial purpose. Commercial use is not allowed.
