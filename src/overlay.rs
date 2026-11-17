//! Non-invasive per-game MangoHud launch options; no global configuration is edited.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayPosition {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl OverlayPosition {
    pub fn from_index(index: i32) -> Self {
        match index {
            1 => Self::TopRight,
            2 => Self::BottomLeft,
            3 => Self::BottomRight,
            _ => Self::TopLeft,
        }
    }
    pub fn index(self) -> i32 {
        match self {
            Self::TopLeft => 0,
            Self::TopRight => 1,
            Self::BottomLeft => 2,
            Self::BottomRight => 3,
        }
    }
    fn mangohud_name(self) -> &'static str {
        match self {
            Self::TopLeft => "top-left",
            Self::TopRight => "top-right",
            Self::BottomLeft => "bottom-left",
            Self::BottomRight => "bottom-right",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlaySettings {
    pub enabled: bool,
    pub position: OverlayPosition,
    pub font_size: u8,
    pub opacity: u8,
    pub fps: bool,
    pub frametime: bool,
    pub cpu: bool,
    pub gpu: bool,
    pub temperatures: bool,
    pub memory: bool,
    pub gpu_power: bool,
    pub opengl_dlsym: bool,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            position: OverlayPosition::TopLeft,
            font_size: 24,
            opacity: 50,
            fps: true,
            frametime: true,
            cpu: true,
            gpu: true,
            temperatures: true,
            memory: true,
            gpu_power: false,
            opengl_dlsym: false,
        }
    }
}

impl OverlaySettings {
    pub fn validate(&mut self) {
        self.font_size = self.font_size.clamp(16, 48);
        self.opacity = self.opacity.min(100);
    }

    /// The target is launch syntax, never executed by Voltix. Steam expands its
    /// placeholder; standalone commands keep their original arguments/quoting.
    pub fn launch_options(&self, target: &str) -> Result<String, String> {
        let target = target.trim();
        if target.is_empty() || target.contains(['\n', '\r', '\0']) {
            return Err("Enter a single-line game command or %command% for Steam".into());
        }
        if target.contains("%command%") && target != "%command%" {
            return Err("Use %command% alone for Steam, or enter a standalone game command".into());
        }
        let options = self.steam_launch_options();
        Ok(format!(
            "{}{target}",
            options.strip_suffix("%command%").unwrap()
        ))
    }

    /// Steam launch options. Only fixed, validated options enter the shell string.
    pub fn steam_launch_options(&self) -> String {
        if !self.enabled {
            return "%command%".to_string();
        }
        let mut s = self.clone();
        s.validate();
        format!("MANGOHUD_CONFIG=fps={},frame_timing={},frametime={},cpu_stats={},gpu_stats={},cpu_temp={},gpu_temp={},ram={},vram={},gpu_power={},position={},font_size={},alpha={:.2},background_alpha={:.2} mangohud{} %command%",
            s.fps as u8, s.frametime as u8, s.frametime as u8, s.cpu as u8, s.gpu as u8,
            s.temperatures as u8, s.temperatures as u8, s.memory as u8, s.memory as u8,
            s.gpu_power as u8, s.position.mangohud_name(), s.font_size, f32::from(s.opacity) / 100.0, f32::from(s.opacity) / 100.0, if s.opengl_dlsym { " --dlsym" } else { "" })
    }
}

/// Use the session's native clipboard, not PRIMARY selection or an XWayland
/// fallback. The helpers retain ownership after Voltix exits and report errors.
pub fn copy_launch_command(text: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let mut command = if std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty()) {
            let mut c = clipboard_command("wl-copy");
            c.args(["--type", "text/plain;charset=utf-8"]);
            c
        } else if std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty()) {
            let mut c = clipboard_command("xclip");
            c.args(["-selection", "clipboard", "-in"]);
            c
        } else {
            return Err("No Wayland or X11 display is available".into());
        };
        write_clipboard(&mut command, text)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = text;
        Err("Clipboard export is supported on Linux only".into())
    }
}

#[cfg(target_os = "linux")]
fn clipboard_command(name: &str) -> std::process::Command {
    // AppImages carry helpers; development builds use the system PATH.
    let bundled = std::env::var_os("APPDIR")
        .map(|root| std::path::PathBuf::from(root).join("usr/bin").join(name));
    match bundled.filter(|path| path.is_file()) {
        Some(path) => std::process::Command::new(path),
        None => std::process::Command::new(name),
    }
}

#[cfg(target_os = "linux")]
fn write_clipboard(command: &mut std::process::Command, text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("Install wl-clipboard (Wayland) or xclip (X11): {err}"))?;
    let write_result = child.stdin.take().unwrap().write_all(text.as_bytes());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                write_result.map_err(|err| format!("Clipboard write failed: {err}"))?;
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!(
                        "Clipboard helper failed ({status}); check display access"
                    ))
                };
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Clipboard helper did not complete: {result:?}"));
            }
        }
    }
}

/// Detect the executable only; this does not prove a working Vulkan/OpenGL layer.
pub fn mangohud_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                std::fs::metadata(dir.join("mangohud"))
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            })
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_metrics_are_explicit_in_launch_options() {
        let config = OverlaySettings {
            enabled: true,
            fps: false,
            frametime: false,
            cpu: false,
            gpu: false,
            temperatures: false,
            memory: false,
            gpu_power: false,
            opengl_dlsym: false,
            ..OverlaySettings::default()
        };
        let command = config.steam_launch_options();
        for disabled in [
            "fps=0",
            "frame_timing=0",
            "frametime=0",
            "cpu_stats=0",
            "gpu_stats=0",
            "cpu_temp=0",
            "gpu_temp=0",
            "ram=0",
            "vram=0",
            "gpu_power=0",
        ] {
            assert!(command
                .split(',')
                .any(|part| part == disabled || part == format!("MANGOHUD_CONFIG={disabled}")));
        }
    }

    #[test]
    fn launch_options_are_explicit_bounded_and_disabled_by_default() {
        let mut overlay = OverlaySettings::default();
        assert_eq!(overlay.steam_launch_options(), "%command%");
        overlay.enabled = true;
        overlay.fps = false;
        overlay.frametime = false;
        overlay.position = OverlayPosition::BottomRight;
        overlay.font_size = 255;
        overlay.opacity = 255;
        let command = overlay.steam_launch_options();
        assert!(command.contains("fps=0,frame_timing=0,frametime=0"));
        assert!(
            command.contains("position=bottom-right,font_size=48,alpha=1.00,background_alpha=1.00")
        );
        assert!(command.ends_with("mangohud %command%"));
        overlay.opengl_dlsym = true;
        assert!(overlay
            .steam_launch_options()
            .ends_with("mangohud --dlsym %command%"));
        let encoded = serde_json::to_string(&overlay).unwrap();
        let decoded: OverlaySettings = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, overlay);
    }
    #[test]
    fn targets_preserve_settings_and_shell_arguments() {
        let overlay = OverlaySettings {
            enabled: true,
            position: OverlayPosition::TopRight,
            fps: false,
            ..Default::default()
        };
        let before = overlay.clone();
        let steam = overlay.launch_options("%command%").unwrap();
        let standalone = overlay
            .launch_options("'/games/My Game' --flag 'a b'")
            .unwrap();
        assert!(steam.contains("fps=0"));
        assert!(steam.contains("position=top-right"));
        assert_eq!(
            standalone,
            steam.replace("%command%", "'/games/My Game' --flag 'a b'")
        );
        assert_eq!(overlay, before);
        for target in ["", " ", "vkcube\nrm -rf /", "vkcube\0", "%command% foo"] {
            assert!(overlay.launch_options(target).is_err());
        }
        assert_eq!(
            OverlaySettings::default().launch_options("vkcube").unwrap(),
            "vkcube"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn clipboard_helper_reports_failure_and_receives_exact_text() {
        use std::process::Command;
        let text = "MANGOHUD_CONFIG=fps=1 mangohud %command%";
        let mut success = Command::new("sh");
        success.args([
            "-c",
            "IFS= read -r value; test \"$value\" = \"$1\"",
            "sh",
            text,
        ]);
        assert!(write_clipboard(&mut success, text).is_ok());
        let mut failure = Command::new("sh");
        failure.args(["-c", "cat >/dev/null; exit 1"]);
        assert!(write_clipboard(&mut failure, text).is_err());
        assert!(write_clipboard(
            &mut Command::new("/nonexistent/voltix-clipboard-helper"),
            text
        )
        .is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires a live Wayland/X11 session and native clipboard helpers"]
    fn native_clipboard_round_trip() {
        let text = OverlaySettings {
            enabled: true,
            position: OverlayPosition::TopRight,
            ..Default::default()
        }
        .launch_options("vkcube")
        .unwrap();
        copy_launch_command(&text).unwrap();
        let mut reader = if std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty()) {
            let mut c = std::process::Command::new("wl-paste");
            c.args(["--no-newline"]);
            c
        } else {
            let mut c = std::process::Command::new("xclip");
            c.args(["-selection", "clipboard", "-out"]);
            c
        };
        let output = reader.output().unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), text);
    }
}
