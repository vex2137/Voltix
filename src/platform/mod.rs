#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "android")]
pub mod android_bridge;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub mod renderer;

pub fn detect_system_dark_mode() -> bool {
    #[cfg(target_os = "linux")]
    {
        linux::system_theme::is_dark_theme_preferred()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

pub mod gpu_probe;
