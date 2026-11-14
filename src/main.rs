mod app;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    if let Err(err) = app::run() {
        log::error!("Voltix exited with UI error: {err}");
        std::process::exit(1);
    }
}
