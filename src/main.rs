fn main() {
    voltix::diagnostics::mark("process entry");
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    if let Err(err) = voltix::app::run(None) {
        log::error!("Voltix exited with UI error: {err}");
        std::process::exit(1);
    }
}
