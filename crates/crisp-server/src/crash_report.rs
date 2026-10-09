use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Local;

#[cfg(windows)]
const LINE_SEPARATOR: &str = "\r\n";
#[cfg(not(windows))]
const LINE_SEPARATOR: &str = "\n";

pub struct CrashReportCategory {
    title: String,
    entries: Vec<(String, String)>,
}

impl CrashReportCategory {
    pub fn set_detail(&mut self, key: &str, value: impl Into<String>) -> &mut CrashReportCategory {
        self.entries.push((key.to_string(), value.into()));
        self
    }

    fn get_details(&self, builder: &mut String) {
        builder.push_str("-- ");
        builder.push_str(&self.title);
        builder.push_str(" --\n");
        builder.push_str("Details:");
        for (key, value) in &self.entries {
            builder.push_str("\n\t");
            builder.push_str(key);
            builder.push_str(": ");
            builder.push_str(value);
        }
    }
}

pub struct CrashReport {
    title: String,
    exception: String,
    stack_trace: Vec<String>,
    details: Vec<CrashReportCategory>,
    system_report: Vec<(String, String)>,
}

impl CrashReport {
    pub fn new(title: &str, exception: String, stack_trace: Vec<String>) -> CrashReport {
        CrashReport { title: title.to_string(), exception, stack_trace, details: Vec::new(), system_report: system_report() }
    }

    pub fn add_category(&mut self, name: &str) -> &mut CrashReportCategory {
        self.details.push(CrashReportCategory { title: name.to_string(), entries: Vec::new() });
        self.details.last_mut().unwrap()
    }

    pub fn set_system_detail(&mut self, key: &str, value: impl Into<String>) {
        self.system_report.push((key.to_string(), value.into()));
    }

    fn exception_message(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.exception);
        out.push_str(LINE_SEPARATOR);
        for frame in &self.stack_trace {
            out.push_str("\tat ");
            out.push_str(frame);
            out.push_str(LINE_SEPARATOR);
        }
        out
    }

    pub fn get_friendly_report(&self) -> String {
        let mut builder = String::new();
        builder.push_str("---- Minecraft Crash Report ----\n");
        builder.push_str("// ");
        builder.push_str(error_comment());
        builder.push_str("\n\n");
        builder.push_str("Time: ");
        builder.push_str(&Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        builder.push('\n');
        builder.push_str("Description: ");
        builder.push_str(&self.title);
        builder.push_str("\n\n");
        builder.push_str(&self.exception_message());
        builder.push_str("\n\nA detailed walkthrough of the error, its code path and all known details is as follows:\n");
        builder.push_str(&"-".repeat(87));
        builder.push_str("\n\n");
        for category in &self.details {
            category.get_details(&mut builder);
            builder.push_str("\n\n");
        }
        builder.push_str("-- System Details --\n");
        builder.push_str("Details:");
        for (key, value) in &self.system_report {
            builder.push_str("\n\t");
            builder.push_str(key);
            builder.push_str(": ");
            builder.push_str(value);
        }
        builder
    }

    pub fn save_to_file(&self, file: &Path) -> bool {
        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(file, self.get_friendly_report()) {
            Ok(()) => true,
            Err(_) => {
                error!("Could not save crash report to {}", file.display());
                false
            }
        }
    }
}

fn error_comment() -> &'static str {
    let nuggets = crisp_data::CRASH_REPORT_NUGGETS;
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    nuggets.get((nanos % nuggets.len().max(1) as u128) as usize).copied().unwrap_or("Witty comment unavailable :(")
}

pub fn crash_report_file(server_directory: &Path) -> std::path::PathBuf {
    server_directory.join("crash-reports").join(format!("crash-{}-server.txt", Local::now().format("%Y-%m-%d_%H.%M.%S")))
}

fn system_report() -> Vec<(String, String)> {
    vec![
        ("Minecraft Version".into(), crisp_data::VERSION_NAME.into()),
        ("Minecraft Version ID".into(), crisp_data::VERSION_NAME.into()),
        ("Operating System".into(), operating_system()),
        ("CPUs".into(), std::thread::available_parallelism().map_or(1, |n| n.get()).to_string()),
    ]
}

fn operating_system() -> String {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "x86" => "x86",
        "aarch64" => "aarch64",
        other => other,
    };
    let name = match std::env::consts::OS {
        "windows" => "Windows",
        "linux" => "Linux",
        "macos" => "Mac OS X",
        other => other,
    };
    format!("{name} ({arch})")
}
