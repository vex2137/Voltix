//! File-backed MangoHud 0.8.4 manager. No game execution or synthetic key injection.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub const METRICS: &[&str] = &[
    "fps",
    "frametime",
    "frame_timing",
    "cpu_stats",
    "gpu_stats",
    "cpu_temp",
    "gpu_temp",
    "ram",
    "vram",
    "gpu_power",
    "cpu_power",
    "cpu_mhz",
    "core_load",
    "gpu_core_clock",
    "gpu_mem_clock",
    "gpu_mem_temp",
    "gpu_junction_temp",
    "gpu_fan",
    "gpu_voltage",
    "swap",
    "procmem",
    "io_read",
    "io_write",
    "battery",
    "battery_watt",
    "network",
    "wine",
    "engine_version",
    "resolution",
    "time",
    "horizontal",
    "no_small_font",
];
pub const APPEARANCE: &[&str] = &[
    "position",
    "font_size",
    "font_scale",
    "font_file",
    "alpha",
    "background_alpha",
    "text_color",
    "background_color",
    "cpu_color",
    "gpu_color",
    "ram_color",
    "vram_color",
    "frametime_color",
    "width",
    "height",
    "table_columns",
    "legacy_layout",
    "reload_cfg",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub config: String,
    pub game: String,
    pub target: Option<PathBuf>,
}
impl Default for Profile {
    fn default() -> Self {
        Self { config: "fps=1\nframetime=1\ncpu_stats=1\ngpu_stats=1\nram=1\nvram=1\nposition=top-right\nfont_size=24\nfont_scale=1.0\nalpha=1.0\nbackground_alpha=0.5\ntext_color=FFFFFF\nbackground_color=020202\nreload_cfg=Shift_L+F4\n".into(), game: "%command%".into(), target: None }
    }
}

pub fn parse(text: &str) -> Result<BTreeMap<String, String>, String> {
    if text.len() > 262144 {
        return Err("Configuration exceeds 256 KiB".into());
    }
    let mut values = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once('=').unwrap_or((line, "1"));
        let (key, value) = (key.trim(), value.trim());
        validate(key, value)?;
        values.insert(key.to_string(), value.to_string());
    }
    Ok(values)
}
fn validate(key: &str, value: &str) -> Result<(), String> {
    let valid = if METRICS.contains(&key) || key == "legacy_layout" {
        matches!(value, "0" | "1")
    } else if key.ends_with("_color") && APPEARANCE.contains(&key) {
        value.len() == 6 && value.bytes().all(|c| c.is_ascii_hexdigit())
    } else {
        match key {
            "position" => [
                "top-left",
                "top-right",
                "bottom-left",
                "bottom-right",
                "top-center",
                "bottom-center",
                "middle-left",
                "middle-right",
            ]
            .contains(&value),
            "font_size" => value.parse::<u32>().is_ok_and(|v| (8..=96).contains(&v)),
            "font_scale" => value
                .parse::<f32>()
                .is_ok_and(|v| (0.25..=4.0).contains(&v)),
            "alpha" | "background_alpha" => {
                value.parse::<f32>().is_ok_and(|v| (0.0..=1.0).contains(&v))
            }
            "table_columns" => value.parse::<u32>().is_ok_and(|v| (1..=8).contains(&v)),
            "width" | "height" => value.parse::<u32>().is_ok_and(|v| v <= 4096),
            "font_file" => {
                !value.is_empty()
                    && Path::new(value).is_absolute()
                    && (value.ends_with(".ttf") || value.ends_with(".otf"))
            }
            "reload_cfg" => value == "Shift_L+F4",
            _ => false,
        }
    };
    if valid && !value.contains(['\n', '\r', '\0']) {
        Ok(())
    } else {
        Err(format!("Unsupported option or invalid value: {key}={value}. See MangoHud 0.8.4 documentation; sensor support varies by driver."))
    }
}
pub fn set_option(text: &str, key: &str, value: &str) -> Result<String, String> {
    validate(key, value)?;
    let mut lines: Vec<String> = text
        .lines()
        .filter(|line| line.trim().split('=').next().unwrap_or("").trim() != key)
        .map(str::to_string)
        .collect();
    lines.push(format!("{key}={value}"));
    let result = format!("{}\n", lines.join("\n"));
    parse(&result)?;
    Ok(result)
}
fn read(path: &Path) -> Result<String, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 262144 {
        return Err("File exceeds 256 KiB".into());
    }
    fs::read_to_string(path).map_err(|e| e.to_string())
}
fn existing(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() && !m.file_type().is_symlink() && m.len() <= 262144 => {
            fs::read(path).map(Some).map_err(|e| e.to_string())
        }
        Ok(_) => Err("Refusing symlink, non-file, or oversized destination".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
fn unique_write(path: &Path, bytes: &[u8], suffix: &str) -> Result<PathBuf, String> {
    for n in 0..10000 {
        let candidate = path.with_file_name(format!(
            "{}.voltix-{suffix}-{n}",
            path.file_name()
                .ok_or("Missing filename")?
                .to_string_lossy()
        ));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&candidate) {
            Ok(mut f) => {
                f.write_all(bytes)
                    .and_then(|_| f.sync_all())
                    .map_err(|e| e.to_string())?;
                return Ok(candidate);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("Too many backup files".into())
}
/// Compare the reviewed bytes before replacing; originals always receive a backup.
pub fn replace(
    path: &Path,
    bytes: &[u8],
    reviewed: &Option<Vec<u8>>,
) -> Result<Option<PathBuf>, String> {
    if existing(path)? != *reviewed {
        return Err("Destination changed; review it again before confirming".into());
    }
    fs::create_dir_all(path.parent().ok_or("Missing parent directory")?)
        .map_err(|e| e.to_string())?;
    let backup = reviewed
        .as_ref()
        .map(|old| unique_write(path, old, "backup"))
        .transpose()?;
    let temp = unique_write(path, bytes, "tmp")?;
    if existing(path)? != *reviewed {
        let _ = fs::remove_file(&temp);
        return Err("Destination changed during save; retry review".into());
    }
    if let Err(e) = fs::rename(&temp, path) {
        let _ = fs::remove_file(temp);
        return Err(e.to_string());
    }
    if fs::read(path).map_err(|e| format!("Unable to verify written file: {e}"))? != bytes {
        return Err("Written file differs from the reviewed content; backup retained".into());
    }
    Ok(backup)
}
#[derive(Clone)]
struct Pending {
    configuration: bool,
    path: PathBuf,
    old: Option<Vec<u8>>,
    new: Vec<u8>,
}
#[derive(Clone)]
pub struct Manager {
    pub profile: Profile,
    pub directory: PathBuf,
    pub target: PathBuf,
    pub status: String,
    pub review_before: String,
    pub review_after: String,
    pending: Option<Pending>,
}
impl Manager {
    pub fn new(config_root: PathBuf) -> Self {
        Self { review_before: String::new(), review_after: String::new(), profile: Profile::default(), directory: config_root.join("voltix/mangohud-profiles"), target: config_root.join("MangoHud/MangoHud.conf"), status: "MangoHud 0.8.4 options. GPU voltage: AMD only; temperatures, power and fan readings depend on drivers. Install MangoHud (including 32-bit libraries for 32-bit games).".into(), pending: None }
    }
    fn profile_path(&self, name: &str) -> Result<PathBuf, String> {
        if name.is_empty()
            || name.len() > 80
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
        {
            return Err("Profile names: 1–80 letters, digits, - or _".into());
        }
        Ok(self.directory.join(format!("{name}.json")))
    }
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(&self.directory)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .filter_map(|e| {
                e.path()
                    .file_stem()
                    .map(|n| n.to_string_lossy().into_owned())
            })
            .collect();
        names.sort();
        names
    }
    pub fn action(
        &mut self,
        action: &str,
        name: &str,
        path: &str,
        content: &str,
        confirmed: bool,
    ) -> Result<String, String> {
        if action != "confirm" {
            self.pending = None;
            self.review_before.clear();
            self.review_after.clear();
        }
        match action {
            "option" => {
                self.profile.config = set_option(&self.profile.config, name, content)?;
            }
            "edit" => {
                parse(content)?;
                self.profile.config = content.into();
                return Ok("Validated editor draft. No file was written.".into());
            }
            "game" => {
                if content.trim().is_empty() || content.contains(['\n', '\r', '\0']) {
                    return Err("Enter a single-line game command or %command%".into());
                }
                self.profile.game = content.into();
            }
            "save" | "duplicate" => {
                self.profile.target = Some(self.target.clone());
                let target = self.profile_path(name)?;
                let old = existing(&target)?;
                if old.is_some() {
                    return Err("Profile exists. Use Review save to overwrite with confirmation, or choose a new name.".into());
                }
                replace(
                    &target,
                    &serde_json::to_vec_pretty(&self.profile).map_err(|e| e.to_string())?,
                    &None,
                )?;
                return Ok(format!(
                    "Profile {name} saved. MangoHud configuration was not changed."
                ));
            }
            "review-save" => {
                self.profile.target = Some(self.target.clone());
                let target = self.profile_path(name)?;
                self.pending = Some(Pending {
                    configuration: false,
                    old: existing(&target)?,
                    path: target,
                    new: serde_json::to_vec_pretty(&self.profile).map_err(|e| e.to_string())?,
                });
                return Ok("Review profile replacement. Confirm writes the edited profile and backs up any previous version.".into());
            }
            "load" => {
                let profile: Profile = serde_json::from_str(&read(&self.profile_path(name)?)?)
                    .map_err(|e| e.to_string())?;
                parse(&profile.config)?;
                if let Some(target) = &profile.target {
                    if !target.is_absolute() {
                        return Err("Profile launch-file path must be absolute".into());
                    }
                    self.target = target.clone();
                }
                self.profile = profile;
                return Ok(format!(
                    "Loaded profile {name} into the editor. No MangoHud file was changed."
                ));
            }
            "delete" => {
                let target = self.profile_path(name)?;
                if !confirmed {
                    return Err("Tick the confirmation box before deleting this profile".into());
                }
                let old = existing(&target)?.ok_or("Profile not found")?;
                unique_write(&target, &old, "backup")?;
                fs::remove_file(target).map_err(|e| e.to_string())?;
                return Ok(format!("Deleted profile {name}; backup retained. MangoHud configuration was not changed."));
            }
            "import" => {
                let text = read(Path::new(path))?;
                parse(&text)?;
                self.profile.config = text;
                return Ok(
                    "Imported configuration into the editor only. Review and confirm to apply it."
                        .into(),
                );
            }
            "review" | "export" => {
                if !content.is_empty() {
                    parse(content)?;
                    self.profile.config = content.into();
                }
                parse(&self.profile.config)?;
                let target = PathBuf::from(path);
                if !target.is_absolute() {
                    return Err("Enter an absolute configuration file path".into());
                }
                let old = existing(&target)?;
                self.review_before = old
                    .as_ref()
                    .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
                    .unwrap_or_default();
                self.review_after = self.profile.config.clone();
                self.pending = Some(Pending {
                    configuration: true,
                    path: target.clone(),
                    old,
                    new: self.profile.config.as_bytes().to_vec(),
                });
                return Ok(format!("Review write to {}. Existing bytes will be backed up. Confirm only after reviewing the editor and destination. Nothing written yet.", target.display()));
            }
            "confirm" => {
                if !confirmed {
                    return Err("Explicit confirmation is required".into());
                }
                if (!content.is_empty() || !path.is_empty()) && content != self.profile.config {
                    self.pending = None;
                    return Err("Editor changed after review; review the destination again".into());
                }
                let pending = self.pending.take().ok_or("Review the destination first")?;
                if !path.is_empty() && pending.configuration && pending.path != Path::new(path) {
                    return Err("Destination changed; review it again before confirming".into());
                }
                let backup = replace(&pending.path, &pending.new, &pending.old)?;
                return Ok(format!("Saved {}. Backup: {}. In an already hooked game MangoHud 0.8.4 watches its active file and can reload automatically; Shift+F4 is the manual fallback. Library/API injection, Proton/runtime or launch-command changes require a game restart. Environment MANGOHUD_CONFIG overrides file settings.", pending.path.display(), backup.map_or("not needed (new file)".into(), |p| p.display().to_string())));
            }
            "copy" => {
                parse(&self.profile.config)?;
                let file = self.target.to_string_lossy();
                let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
                if self.profile.game.contains("%command%") && self.profile.game != "%command%" {
                    return Err("Use %command% alone for Steam/Proton".into());
                }
                let command = format!(
                    "MANGOHUD_CONFIGFILE={} mangohud {}",
                    quote(&file),
                    self.profile.game
                );
                super::overlay::copy_launch_command(&command)?;
                return Ok("Copied to clipboard. Apply this profile to the launch file first. Steam/Proton: %command%; Vulkan and OpenGL: mangohud wrapper (dlsym is enabled by default in 0.8.4).".into());
            }
            "target" => {
                let target = PathBuf::from(path);
                if !target.is_absolute() {
                    return Err("Use an absolute launch-file path".into());
                }
                self.target = target.clone();
                self.profile.target = Some(target);
                return Ok(format!(
                    "Selected launch file {}. This does not apply the editor configuration.",
                    self.target.display()
                ));
            }
            "refresh" => {}
            _ => return Err("Unknown manager action".into()),
        }
        Ok("Updated. Preview is illustrative; apply the file, MangoHud 0.8.4 can auto-reload its active file; Shift+F4 is the fallback.".into())
    }
    pub fn preview(&self) -> String {
        let values = parse(&self.profile.config).unwrap_or_default();
        METRICS
            .iter()
            .filter(|key| values.get(**key).is_some_and(|v| v == "1"))
            .map(|key| match *key {
                "fps" => "FPS   144 (sample)",
                "frametime" => "Frametime   6.9 ms (sample)",
                other => other,
            })
            .collect::<Vec<_>>()
            .join(if values.get("horizontal").is_some_and(|v| v == "1") {
                "  |  "
            } else {
                "\n"
            })
    }
}

/// Best-effort discovery only; actual ABI/layer/sensor availability is a game-time check.
pub fn components() -> String {
    let vulkan = [
        "/usr/share/vulkan/implicit_layer.d",
        "/usr/local/share/vulkan/implicit_layer.d",
    ]
    .iter()
    .any(|dir| {
        fs::read_dir(dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .contains("mangohud")
            })
    });
    let opengl = [
        "/usr/lib/mangohud/libMangoHud_opengl.so",
        "/usr/lib64/mangohud/libMangoHud_opengl.so",
        "/usr/lib/x86_64-linux-gnu/mangohud/libMangoHud_opengl.so",
    ]
    .iter()
    .any(|p| Path::new(p).is_file());
    format!("Vulkan manifest: {}; OpenGL library: {}. Standard paths only; custom/Flatpak installations and 32-bit compatibility are not verified.", if vulkan { "found" } else { "not found" }, if opengl { "found" } else { "not found" })
}
