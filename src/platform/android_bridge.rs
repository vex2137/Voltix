//! JNI receives immutable foreground snapshots; collectors never block on Java/UI calls.
use crate::monitoring::extended::Platform;
use jni::{
    objects::{JClass, JString},
    sys::{jint, jstring},
    JNIEnv,
};
use std::sync::{
    atomic::{AtomicI32, Ordering},
    Mutex, OnceLock,
};
use std::time::{Duration, Instant};
static SAMPLE: OnceLock<Mutex<Option<(Instant, Platform)>>> = OnceLock::new();
static EXPORT: OnceLock<Mutex<String>> = OnceLock::new();
static COMMAND: AtomicI32 = AtomicI32::new(0);
static POLLING: AtomicI32 = AtomicI32::new(1000);
pub fn snapshot() -> Platform {
    SAMPLE
        .get_or_init(Default::default)
        .lock()
        .ok()
        .and_then(|sample| {
            sample
                .as_ref()
                .filter(|(time, _)| time.elapsed() < Duration::from_secs(15))
                .map(|(_, p)| p.clone())
        })
        .unwrap_or_default()
}
pub fn request_settings() {
    COMMAND.store(1, Ordering::Release);
}
pub fn export(json: String) {
    if let Ok(mut data) = EXPORT.get_or_init(Default::default).lock() {
        *data = json;
        COMMAND.store(2, Ordering::Release);
    }
}
pub fn polling(ms: u64) {
    POLLING.store(ms.clamp(1000, 5000) as i32, Ordering::Relaxed);
}
#[no_mangle]
pub extern "system" fn Java_com_voltix_monitor_VoltixActivity_nativeSnapshot(
    mut env: JNIEnv,
    _class: JClass,
    json: JString,
) {
    let Ok(text) = env.get_string(&json) else {
        return;
    };
    let text: String = text.into();
    if text.len() > 262144 {
        return;
    }
    if let Ok(data) = serde_json::from_str::<Platform>(&text) {
        if let Ok(mut value) = SAMPLE.get_or_init(Default::default).lock() {
            *value = Some((Instant::now(), data));
        }
    }
}
#[no_mangle]
pub extern "system" fn Java_com_voltix_monitor_VoltixActivity_nativeCommand(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    COMMAND.swap(0, Ordering::AcqRel)
}
#[no_mangle]
pub extern "system" fn Java_com_voltix_monitor_VoltixActivity_nativePollingInterval(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    POLLING.load(Ordering::Relaxed)
}
#[no_mangle]
pub extern "system" fn Java_com_voltix_monitor_VoltixActivity_nativeExportJson(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let data = EXPORT
        .get_or_init(Default::default)
        .lock()
        .map(|v| v.clone())
        .unwrap_or_default();
    env.new_string(data)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
