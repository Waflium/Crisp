use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle, Thread};
use std::time::SystemTime;

use chrono::{DateTime, Local, NaiveDate};
use flate2::Compression;
use flate2::write::GzEncoder;

const MAX_FILES_PER_DAY: u32 = 7;
const BUFFER_SIZE: usize = 64 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl Level {
    fn name(self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
            Level::Fatal => "FATAL",
        }
    }
}

enum ThreadName {
    Current(Thread),
    Named(String),
}

struct Record {
    time: SystemTime,
    thread: ThreadName,
    level: Level,
    message: String,
}

enum Message {
    Record(Record),
    Flush(Sender<()>),
}

struct Logger {
    queue: Sender<Message>,
    writer: Mutex<Option<JoinHandle<()>>>,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

pub fn init(dir: impl Into<PathBuf>) {
    let dir = dir.into();
    let (queue, records) = channel();
    let writer = thread::Builder::new().name("Log4j2-AsyncAppender".into()).spawn(move || write_records(&dir, records)).expect("log writer thread");
    let _ = LOGGER.set(Logger { queue, writer: Mutex::new(Some(writer)) });
}

pub fn flush() {
    if let Some(logger) = LOGGER.get() {
        let (done, wait) = channel();
        if logger.queue.send(Message::Flush(done)).is_ok() {
            let _ = wait.recv();
        }
    }
}

pub fn shutdown() {
    flush();
    if let Some(logger) = LOGGER.get() {
        logger.writer.lock().unwrap().take();
    }
}

pub fn log(level: Level, args: fmt::Arguments) {
    submit(level, ThreadName::Current(thread::current()), args);
}

pub fn log_as(thread: &str, level: Level, args: fmt::Arguments) {
    submit(level, ThreadName::Named(thread.to_string()), args);
}

fn submit(level: Level, thread: ThreadName, args: fmt::Arguments) {
    if level < Level::Info {
        return;
    }
    let record = Record { time: SystemTime::now(), thread, level, message: fmt::format(args) };
    let Some(logger) = LOGGER.get() else {
        return print_now(&record);
    };
    if let Err(rejected) = logger.queue.send(Message::Record(record))
        && let Message::Record(record) = rejected.0
    {
        print_now(&record);
    }
}

fn print_now(record: &Record) {
    let mut line = Vec::with_capacity(record.message.len() + 40);
    format_record(&mut line, record);
    print!("{}", String::from_utf8_lossy(&line));
}

fn format_record(out: &mut Vec<u8>, record: &Record) {
    let time: DateTime<Local> = record.time.into();
    let thread_name = match &record.thread {
        ThreadName::Current(thread) => thread.name().unwrap_or(""),
        ThreadName::Named(name) => name,
    };
    let _ = write!(out, "[{}] [{thread_name}/{}]: {}", time.format("%H:%M:%S"), record.level.name(), record.message);
    out.push(b'\n');
}

fn write_records(dir: &Path, queue: Receiver<Message>) {
    let mut file = RollingFile::open(dir);
    let mut stdout = BufWriter::with_capacity(BUFFER_SIZE, io::stdout());
    let mut line = Vec::with_capacity(256);
    let mut next = queue.recv().ok();
    while let Some(message) = next {
        match message {
            Message::Record(record) => {
                line.clear();
                format_record(&mut line, &record);
                let _ = stdout.write_all(&line);
                file.write(&line, record.time);
            }
            Message::Flush(done) => {
                let _ = stdout.flush();
                file.flush();
                let _ = done.send(());
            }
        }
        next = queue.try_recv().ok().or_else(|| {
            let _ = stdout.flush();
            file.flush();
            queue.recv().ok()
        });
    }
    let _ = stdout.flush();
    file.flush();
}

struct RollingFile {
    dir: PathBuf,
    out: Option<BufWriter<File>>,
    day: NaiveDate,
}

impl RollingFile {
    fn open(dir: &Path) -> RollingFile {
        let _ = fs::create_dir_all(dir);
        let today = Local::now().date_naive();
        let mut file = RollingFile { dir: dir.to_path_buf(), out: None, day: today };
        if let Ok(meta) = fs::metadata(file.latest())
            && meta.len() > 0
        {
            file.roll_over(meta.modified().map_or(today, |t| DateTime::<Local>::from(t).date_naive()));
        }
        file.out = file.create();
        file
    }

    fn latest(&self) -> PathBuf {
        self.dir.join("latest.log")
    }

    fn create(&self) -> Option<BufWriter<File>> {
        File::create(self.latest()).ok().map(|f| BufWriter::with_capacity(BUFFER_SIZE, f))
    }

    fn write(&mut self, line: &[u8], time: SystemTime) {
        let day = DateTime::<Local>::from(time).date_naive();
        if day > self.day {
            self.flush();
            self.out = None;
            self.roll_over(self.day);
            self.day = day;
            self.out = self.create();
        }
        if let Some(out) = &mut self.out {
            let _ = out.write_all(line);
        }
    }

    fn flush(&mut self) {
        if let Some(out) = &mut self.out {
            let _ = out.flush();
        }
    }

    fn roll_over(&self, day: NaiveDate) {
        let index = next_index(&self.dir, day);
        let target = self.dir.join(format!("{}-{index}.log", day.format("%Y-%m-%d")));
        if fs::rename(self.latest(), &target).is_err() {
            return;
        }
        let _ = thread::Builder::new().name("Log4j2-GzCompressAction".into()).spawn(move || {
            if gzip(&target).is_ok() {
                let _ = fs::remove_file(&target);
            }
        });
    }
}

fn next_index(dir: &Path, day: NaiveDate) -> u32 {
    let prefix = format!("{}-", day.format("%Y-%m-%d"));
    let mut existing: Vec<(u32, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let rest = name.strip_prefix(&prefix)?;
            let index = rest.strip_suffix(".log.gz").or_else(|| rest.strip_suffix(".log"))?.parse().ok()?;
            (1..=MAX_FILES_PER_DAY).contains(&index).then(|| (index, entry.path()))
        })
        .collect();
    existing.sort();
    if (existing.len() as u32) < MAX_FILES_PER_DAY {
        return existing.last().map_or(1, |&(i, _)| i + 1).min(MAX_FILES_PER_DAY);
    }
    let excess = existing.len() + 1 - MAX_FILES_PER_DAY as usize;
    for (_, path) in existing.drain(..excess) {
        let _ = fs::remove_file(path);
    }
    for (new_index, (_, path)) in (1..).zip(&existing) {
        let suffix = if path.extension().is_some_and(|e| e == "gz") { ".log.gz" } else { ".log" };
        let _ = fs::rename(path, dir.join(format!("{prefix}{new_index}{suffix}")));
    }
    existing.len() as u32 + 1
}

fn gzip(path: &Path) -> io::Result<()> {
    let mut input = File::open(path)?;
    let mut output = GzEncoder::new(BufWriter::new(File::create(path.with_extension("log.gz"))?), Compression::default());
    io::copy(&mut input, &mut output)?;
    output.finish()?.flush()
}

#[macro_export]
macro_rules! info {
    (thread: $thread:expr, $($arg:tt)*) => { $crate::logging::log_as($thread, $crate::logging::Level::Info, format_args!($($arg)*)) };
    ($($arg:tt)*) => { $crate::logging::log($crate::logging::Level::Info, format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! warn {
    (thread: $thread:expr, $($arg:tt)*) => { $crate::logging::log_as($thread, $crate::logging::Level::Warn, format_args!($($arg)*)) };
    ($($arg:tt)*) => { $crate::logging::log($crate::logging::Level::Warn, format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! error {
    (thread: $thread:expr, $($arg:tt)*) => { $crate::logging::log_as($thread, $crate::logging::Level::Error, format_args!($($arg)*)) };
    ($($arg:tt)*) => { $crate::logging::log($crate::logging::Level::Error, format_args!($($arg)*)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollover_indices_follow_log4j() {
        let dir = std::env::temp_dir().join(format!("crisp-logs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let day = NaiveDate::from_ymd_opt(2026, 10, 8).unwrap();
        assert_eq!(next_index(&dir, day), 1);
        for i in 1..=6u8 {
            fs::write(dir.join(format!("2026-10-08-{i}.log.gz")), [i]).unwrap();
        }
        assert_eq!(next_index(&dir, day), 7);
        fs::write(dir.join("2026-10-08-7.log.gz"), [7]).unwrap();
        assert_eq!(next_index(&dir, day), 7);
        assert_eq!(fs::read(dir.join("2026-10-08-1.log.gz")).unwrap(), [2]);
        assert_eq!(fs::read(dir.join("2026-10-08-6.log.gz")).unwrap(), [7]);
        assert!(!dir.join("2026-10-08-7.log.gz").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
