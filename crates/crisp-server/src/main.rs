use crisp_server::logging;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server_main = std::thread::Builder::new()
        .name("ServerMain".into())
        .spawn(move || {
            logging::init("logs");
            crisp_server::server::main::main(args);
        })
        .expect("server main thread");
    let _ = server_main.join();
    logging::shutdown();
    std::process::exit(0)
}
