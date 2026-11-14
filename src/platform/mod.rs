#[cfg(target_os = "linux")]
pub mod linux;

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
