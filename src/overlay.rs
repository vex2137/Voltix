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
}
