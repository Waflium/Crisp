pub mod network_data_output_stream;
pub mod query_thread_gs4;
pub mod rcon_thread;

use std::sync::atomic::{AtomicUsize, Ordering};

static UNIQUE_THREAD_ID: AtomicUsize = AtomicUsize::new(0);

pub fn start_generic_thread(starter: &str, name: &str) -> String {
    let thread_name = format!("{name} #{}", UNIQUE_THREAD_ID.fetch_add(1, Ordering::Relaxed) + 1);
    info!(thread: starter, "Thread {name} started");
    thread_name
}

pub fn java_inet_address(address: std::net::IpAddr) -> String {
    match address {
        std::net::IpAddr::V4(v4) => format!("/{v4}"),
        std::net::IpAddr::V6(v6) => format!("/{}", v6.segments().map(|s| format!("{s:x}")).join(":")),
    }
}

pub fn java_bind_exception(error: &std::io::Error, datagram: bool) -> String {
    let reason = match error.kind() {
        std::io::ErrorKind::AddrInUse => "Address already in use",
        std::io::ErrorKind::AddrNotAvailable => "Cannot assign requested address",
        std::io::ErrorKind::PermissionDenied => "Permission denied",
        _ => return format!("java.net.SocketException: {error}"),
    };
    match (cfg!(windows), datagram) {
        (true, true) => format!("java.net.BindException: {reason}: Cannot bind"),
        (true, false) => format!("java.net.BindException: {reason}: bind"),
        (false, _) => format!("java.net.BindException: {reason}"),
    }
}
