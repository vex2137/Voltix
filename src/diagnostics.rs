//! Opt-in initialization diagnostics; never equate initialization with presentation.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};
use std::time::Instant;
static START: OnceLock<Instant> = OnceLock::new();
static ENABLED: OnceLock<bool> = OnceLock::new();
static FIRST_SAMPLE: AtomicBool = AtomicBool::new(false);
pub fn mark(stage: &str) {
    if !*ENABLED.get_or_init(|| std::env::var_os("VOLTIX_PROFILE_STARTUP").is_some()) {
        return;
    }
    let start = START.get_or_init(Instant::now);
    eprintln!(
        "voltix startup: {stage}: {:.2} ms",
        start.elapsed().as_secs_f64() * 1000.0
    );
}
pub fn first_sample() {
    if !FIRST_SAMPLE.swap(true, Ordering::Relaxed) {
        mark("first telemetry applied to UI (not frame presentation)");
    }
}
