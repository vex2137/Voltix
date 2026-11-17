fn main() {
    voltix::diagnostics::mark("process entry");
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    #[cfg(target_os = "linux")]
    if let Err(err) = voltix::platform::renderer::initialize() {
        voltix::platform::renderer::retry_after_error(&err);
        log::error!("Failed to initialize a UI backend: {err}");
        std::process::exit(1);
    }

    if let Err(err) = voltix::app::run(None) {
        #[cfg(target_os = "linux")]
        voltix::platform::renderer::retry_after_error(&err);
        log::error!("Voltix exited with UI error: {err}");
        std::process::exit(1);
    }
}
