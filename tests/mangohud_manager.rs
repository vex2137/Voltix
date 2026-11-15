use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use voltix::mangohud::{parse, set_option, Manager};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "voltix-manager-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn curated_config_validates_and_preserves_comments_and_other_options() {
    let config = "# My game\nfps\ngpu_temp=1\nposition=top-right\n";
    let changed = set_option(config, "fps", "0").unwrap();
    assert!(changed.starts_with("# My game\n"));
    let values = parse(&changed).unwrap();
    assert_eq!(values["gpu_temp"], "1");
    assert_eq!(values["fps"], "0");
    for invalid in [
        "exec=touch /tmp/unsafe",
        "font_scale=NaN",
        "alpha=1.1",
        "text_color=red",
        "position=invalid",
        "columns=3",
        "fps=1\nexec=command",
    ] {
        assert!(parse(invalid).is_err(), "{invalid}");
    }
    assert!(
        parse("font_file=/fonts/My Font.ttf\nfont_scale=1.5\nhorizontal=1\ntable_columns=4")
            .is_ok()
    );
}
#[test]
fn profiles_roundtrip_duplicate_edit_backup_delete_and_per_game_path() {
    let root = Workspace::new();
    let mut m = Manager::new(root.0.clone());
    let game_file = root.0.join("MangoHud/wine-Game.conf");
    m.action("target", "", game_file.to_str().unwrap(), "", false)
        .unwrap();
    m.action("game", "", "", "'/games/Game.exe' --flag", false)
        .unwrap();
    m.action("save", "game", "", "", false).unwrap();
    m.action("duplicate", "copy", "", "", false).unwrap();
    m.action("option", "fps", "", "0", false).unwrap();
    m.action("review-save", "game", "", "", false).unwrap();
    assert!(m.action("confirm", "", "", "", false).is_err());
    m.action("confirm", "", "", "", true).unwrap();
    m.action("load", "copy", "", "", false).unwrap();
    assert_eq!(parse(&m.profile.config).unwrap()["fps"], "1");
    m.action("load", "game", "", "", false).unwrap();
    assert_eq!(parse(&m.profile.config).unwrap()["fps"], "0");
    assert_eq!(m.target, game_file);
    assert_eq!(m.profile.game, "'/games/Game.exe' --flag");
    assert!(m.action("delete", "game", "", "", false).is_err());
    m.action("delete", "game", "", "", true).unwrap();
    assert_eq!(m.names(), vec!["copy"]);
    assert!(m.action("save", "../outside", "", "", false).is_err());
}
#[test]
fn apply_requires_review_confirmation_and_preserves_original_bytes() {
    let root = Workspace::new();
    let mut m = Manager::new(root.0.clone());
    let path = root.0.join("MangoHud.conf");
    let original = b"# existing with unknown option\nexec=something\n";
    fs::write(&path, original).unwrap();
    assert!(m.action("confirm", "", "", "", true).is_err());
    m.action("review", "", path.to_str().unwrap(), "", false)
        .unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
    m.action("confirm", "", "", "", true).unwrap();
    assert_eq!(
        fs::read(path.with_file_name("MangoHud.conf.voltix-backup-0")).unwrap(),
        original
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), m.profile.config);
    m.action("review", "", path.to_str().unwrap(), "", false)
        .unwrap();
    fs::write(&path, "# externally edited").unwrap();
    assert!(m.action("confirm", "", "", "", true).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "# externally edited");
    m.action("review", "", path.to_str().unwrap(), "", false)
        .unwrap();
    m.action("option", "fps", "", "0", false).unwrap();
    assert!(m.action("confirm", "", "", "", true).is_err());
}
#[test]
fn import_export_no_silent_overwrite_and_preview_reflects_layout() {
    let root = Workspace::new();
    let mut m = Manager::new(root.0.clone());
    let source = root.0.join("import.conf");
    fs::write(&source, "fps=1\nram=1\nhorizontal=1\n").unwrap();
    m.action("import", "", source.to_str().unwrap(), "", false)
        .unwrap();
    assert!(m.preview().contains(" | "));
    let output = root.0.join("export.conf");
    m.action("export", "", output.to_str().unwrap(), "", false)
        .unwrap();
    assert!(!output.exists());
    m.action("confirm", "", "", "", true).unwrap();
    assert_eq!(fs::read(source).unwrap(), fs::read(output).unwrap());
}
#[cfg(unix)]
#[test]
fn symlink_destination_is_rejected() {
    let root = Workspace::new();
    let original = root.0.join("original");
    fs::write(&original, "fps=1").unwrap();
    let link = root.0.join("symlink");
    std::os::unix::fs::symlink(&original, &link).unwrap();
    let mut m = Manager::new(root.0.clone());
    assert!(m
        .action("review", "", link.to_str().unwrap(), "", false)
        .is_err());
    assert_eq!(fs::read_to_string(original).unwrap(), "fps=1");
}

#[test]
fn review_uses_visible_editor_and_verifies_selected_destination() {
    let root = Workspace::new();
    let mut m = Manager::new(root.0.clone());
    let path = root.0.join("MangoHud.conf");
    fs::write(&path, "position=top-left\n").unwrap();
    let editor = "position=top-right\n";
    m.action("review", "", path.to_str().unwrap(), editor, false)
        .unwrap();
    assert_eq!(m.review_before, "position=top-left\n");
    assert_eq!(m.review_after, editor);
    assert_eq!(fs::read_to_string(&path).unwrap(), "position=top-left\n");
    assert!(m
        .action("confirm", "", path.to_str().unwrap(), editor, false)
        .is_err());
    m.action("confirm", "", path.to_str().unwrap(), editor, true)
        .unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), editor);
    assert_eq!(
        fs::read_to_string(path.with_file_name("MangoHud.conf.voltix-backup-0")).unwrap(),
        "position=top-left\n"
    );
    // Edits and destination changes made after review cannot silently write stale bytes.
    m.action("review", "", path.to_str().unwrap(), editor, false)
        .unwrap();
    assert!(m
        .action("confirm", "", path.to_str().unwrap(), "", true)
        .is_err());
    m.action("review", "", path.to_str().unwrap(), editor, false)
        .unwrap();
    let other = root.0.join("other.conf");
    assert!(m
        .action("confirm", "", other.to_str().unwrap(), editor, true)
        .is_err());
    assert!(!other.exists());
    assert_eq!(fs::read_to_string(path).unwrap(), editor);
}
