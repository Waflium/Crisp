use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail, ensure};

const VERSION: &str = "26.3";
const SERVER_JAR_SHA1: &str = "33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c";

fn main() -> anyhow::Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("vanilla") => vanilla(),
        Some("worldgen-oracle") => worldgen_oracle(std::env::args().skip(2).collect()),
        _ => bail!("usage: cargo xtask vanilla | cargo xtask worldgen-oracle <mode> <args...>"),
    }
}

fn worldgen_oracle(args: Vec<String>) -> anyhow::Result<()> {
    let dir = workspace_root().join(".vanilla");
    let server_jar = dir.join(format!("versions/{VERSION}/server-{VERSION}.jar"));
    ensure!(server_jar.exists(), "run `cargo xtask vanilla` first");
    let mut classpath = vec![server_jar];
    collect_jars(&dir.join("libraries"), &mut classpath)?;
    let classes = dir.join("oracle-classes");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("oracle/WorldgenOracle.java");
    let status = Command::new("javac")
        .arg("-nowarn")
        .arg("-d")
        .arg(&classes)
        .arg("-cp")
        .arg(std::env::join_paths(&classpath)?)
        .arg(&source)
        .status()
        .context("failed to run javac (a JDK 25 or newer is required)")?;
    ensure!(status.success(), "compiling the oracle failed: {status}");
    classpath.insert(0, classes);
    let status = Command::new("java").current_dir(&dir).arg("-cp").arg(std::env::join_paths(classpath)?).arg("crisp.WorldgenOracle").args(args).status()?;
    ensure!(status.success(), "the oracle failed: {status}");
    Ok(())
}

fn vanilla() -> anyhow::Result<()> {
    let dir = workspace_root().join(".vanilla");
    fs::create_dir_all(&dir)?;
    let jar = dir.join("server.jar");

    if file_sha1(&jar).ok().as_deref() != Some(SERVER_JAR_SHA1) {
        let url = format!("https://piston-data.mojang.com/v1/objects/{SERVER_JAR_SHA1}/server.jar");
        println!("downloading {url}");
        let mut body = ureq::get(&url).call()?.into_body();
        let bytes = body.with_config().limit(256 * 1024 * 1024).read_to_vec()?;
        ensure!(sha1_hex(&bytes) == SERVER_JAR_SHA1, "server.jar checksum mismatch");
        fs::write(&jar, bytes)?;
    }

    println!("running the vanilla data generator");
    let status = Command::new("java")
        .current_dir(&dir)
        .args(["-DbundlerMainClass=net.minecraft.data.Main", "-jar", "server.jar"])
        .args(["--reports", "--output", "generated"])
        .status()
        .context("failed to run java (Java 25 or newer is required)")?;
    ensure!(status.success(), "data generator failed: {status}");

    println!("unpacking the vanilla data pack");
    let inner = dir.join(format!("versions/{VERSION}/server-{VERSION}.jar"));
    let mut zip = zip::ZipArchive::new(fs::File::open(&inner).with_context(|| inner.display().to_string())?)?;
    let data_dir = dir.join("data");
    if data_dir.exists() {
        fs::remove_dir_all(&data_dir)?;
    }
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_owned();
        if !(name.starts_with("data/") || name == "version.json" || name == "assets/minecraft/lang/en_us.json") || entry.is_dir() {
            continue;
        }
        let out = dir.join(&name);
        fs::create_dir_all(out.parent().unwrap())?;
        io::copy(&mut entry, &mut fs::File::create(&out)?)?;
    }

    println!("extracting block and item properties");
    extract(&dir, &inner)?;

    println!("vanilla {VERSION} data is ready in {}", dir.display());
    Ok(())
}

fn extract(dir: &Path, server_jar: &Path) -> anyhow::Result<()> {
    let mut classpath = vec![server_jar.to_owned()];
    collect_jars(&dir.join("libraries"), &mut classpath)?;
    let classpath = std::env::join_paths(&classpath)?;
    let classes = dir.join("extractor-classes");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("extractor/Extract.java");

    let status = Command::new("javac")
        .arg("-nowarn")
        .arg("-d")
        .arg(&classes)
        .arg("-cp")
        .arg(&classpath)
        .arg(&source)
        .status()
        .context("failed to run javac (a JDK 25 or newer is required)")?;
    ensure!(status.success(), "compiling the extractor failed: {status}");

    let mut run_classpath = std::env::split_paths(&classpath).collect::<Vec<_>>();
    run_classpath.insert(0, classes);
    let status = Command::new("java")
        .current_dir(dir)
        .arg("-cp")
        .arg(std::env::join_paths(run_classpath)?)
        .arg("crisp.Extract")
        .arg(dir.join("extracted"))
        .status()?;
    ensure!(status.success(), "the extractor failed: {status}");
    Ok(())
}

fn collect_jars(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_jars(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "jar") {
            out.push(path);
        }
    }
    Ok(())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_owned()
}

fn file_sha1(path: &Path) -> io::Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)?.read_to_end(&mut bytes)?;
    Ok(sha1_hex(&bytes))
}

fn sha1_hex(bytes: &[u8]) -> String {
    sha1_smol::Sha1::from(bytes).digest().to_string()
}
