use voltix::{
    monitoring::processes::{cpu_delta, parse_effective_uid, parse_process, Process, ProcessKey},
    process_ui::{filtered, Filter},
};

#[test]
fn proc_stat_fields_and_effective_uid_are_not_guessed() {
    let mut fields = vec!["0"; 22];
    for (i, v) in [
        (0, "S"),
        (1, "101"),
        (11, "11"),
        (12, "12"),
        (17, "3"),
        (19, "4321"),
        (21, "21"),
    ] {
        fields[i] = v;
    }
    let fields = fields.join(" ");
    let (p, ticks, start) =
        parse_process(42, &format!("42 (name (with) spaces) {fields}"), 4096).unwrap();
    assert_eq!((p.ppid, p.state, p.threads), (101, 'S', Some(3)));
    assert_eq!(
        (ticks, start, p.start_ticks, p.memory),
        (23, 4321, 4321, 21 * 4096)
    );
    assert_eq!(p.name, "name (with) spaces");
    assert_eq!(
        parse_effective_uid("Name: test\nUid:\t1000\t1001\t1002\t1003\n"),
        Some(1001)
    );
    assert_eq!(parse_effective_uid("Uid: invalid\n"), None);
    assert!(parse_process(1, "1 (bad) R 1", 4096).is_none());
}
#[test]
fn cpu_samples_keep_multicore_values_and_reject_resets() {
    assert_eq!(cpu_delta(100, 600, 2., 100), Some(250.));
    assert_eq!(cpu_delta(600, 100, 2., 100), None);
    for seconds in [0., -1., f64::INFINITY, f64::NAN] {
        assert_eq!(cpu_delta(1, 2, seconds, 100), None);
    }
    assert_eq!(cpu_delta(1, 2, 1., 0), None);
    assert_ne!(
        ProcessKey {
            pid: 5,
            start_ticks: 10
        },
        ProcessKey {
            pid: 5,
            start_ticks: 20
        }
    );
    assert_eq!(ProcessKey::decode("5:10").unwrap().encode(), "5:10");
    for key in ["5", "0:1", "5:invalid", "-1:1", "5:1:2"] {
        assert!(ProcessKey::decode(key).is_none());
    }
}
fn fixture() -> Vec<Process> {
    [
        (20, "Alpha", Some(200.), 128, Some(1000)),
        (10, "beta", Some(10.), 512, Some(1001)),
        (30, "ALPINE", None, 256, None),
        (40, "quiet", Some(0.), 1, Some(1000)),
    ]
    .into_iter()
    .map(|(pid, name, cpu_percent, mem, uid)| Process {
        pid,
        name: name.into(),
        cpu_percent,
        memory: mem * 1024 * 1024,
        uid,
        start_ticks: pid as u64,
        ..Default::default()
    })
    .collect()
}
#[test]
fn live_filters_directions_and_unknown_cpu_order_are_consistent() {
    let p = fixture();
    let mut f = Filter::default();
    assert_eq!(
        filtered(&p, &f).iter().map(|p| p.pid).collect::<Vec<_>>(),
        vec![20, 10, 40, 30]
    );
    f.descending = false;
    assert_eq!(
        filtered(&p, &f).iter().map(|p| p.pid).collect::<Vec<_>>(),
        vec![40, 10, 20, 30]
    );
    f.query = " aLp ".into();
    assert_eq!(filtered(&p, &f).len(), 2);
    f.query = "10".into();
    assert_eq!(filtered(&p, &f)[0].name, "beta");
    f.query.clear();
    f.quick = 1;
    assert_eq!(filtered(&p, &f).len(), 2);
    f.quick = 2;
    assert_eq!(filtered(&p, &f).len(), 2);
    f.own_uid = Some(1000);
    assert!(filtered(&p, &f).is_empty());
    f.quick = 0;
    assert_eq!(filtered(&p, &f).len(), 2);
    f.own_uid = None;
    f.sort = 1;
    f.descending = true;
    assert_eq!(filtered(&p, &f)[0].pid, 10);
    f.sort = 2;
    f.descending = false;
    assert_eq!(filtered(&p, &f)[0].pid, 10);
    f.sort = 3;
    assert_eq!(filtered(&p, &f)[0].name, "Alpha");
    f.descending = true;
    assert_eq!(filtered(&p, &f)[0].name, "quiet");
    assert_eq!(p[0].cpu_percent, Some(200.)); // presentation is read-only
}
#[test]
fn lifetime_selection_survives_filtering_and_resource_reordering() {
    let mut p = fixture();
    let selected = p[0].key();
    p[0].cpu_percent = Some(0.);
    p[1].cpu_percent = Some(300.);
    let rows = filtered(&p, &Filter::default());
    assert_ne!(rows[0].key(), selected);
    assert_eq!(p.iter().find(|p| p.key() == selected).unwrap().pid, 20);
    p[0].start_ticks += 1;
    assert!(p.iter().all(|p| p.key() != selected));
}
#[cfg(target_os = "linux")]
#[test]
fn linux_sampling_reads_own_process_and_rejects_reused_identity() {
    use std::time::{Duration, Instant};
    use voltix::monitoring::processes::{ProcessMonitor, PROCESS_LIMIT};
    let pages = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
    let key = parse_process(
        std::process::id(),
        &std::fs::read_to_string("/proc/self/stat").unwrap(),
        pages,
    )
    .unwrap()
    .0
    .key();
    let mut monitor = ProcessMonitor::default();
    let now = Instant::now();
    let first = monitor.sample(now, Some(key));
    assert!(first.available && first.count.unwrap() > 0);
    assert!(first.rows.len() <= PROCESS_LIMIT);
    let d = first.details.unwrap();
    assert_eq!(d.process.key(), key);
    assert_eq!(d.process.uid, Some(unsafe { libc::geteuid() }));
    assert!(d.executable.is_some() && d.command.is_some());
    assert!(d.process.memory > 0 && d.process.threads.unwrap() > 0);
    assert!(first.rows.iter().all(|p| p.cpu_percent.is_none()));
    let second = monitor.sample(now + Duration::from_millis(20), Some(key));
    if !second.truncated {
        assert!(second.details.unwrap().process.cpu_percent.is_some());
    }
    assert!(monitor
        .sample(
            now + Duration::from_millis(40),
            Some(ProcessKey {
                start_ticks: key.start_ticks + 1,
                ..key
            })
        )
        .details
        .is_none());
    monitor.reset();
    assert!(monitor
        .sample(now + Duration::from_secs(1), Some(key))
        .rows
        .iter()
        .all(|p| p.cpu_percent.is_none()));
}

#[test]
fn old_settings_default_new_filters_and_new_preferences_round_trip() {
    use voltix::settings::UserSettings;
    let mut settings: UserSettings = serde_json::from_str(r#"{"process_sort":2}"#).unwrap();
    assert!(settings.process_descending);
    assert_eq!(settings.process_quick_filter, 0);
    assert!(!settings.process_own_user);
    settings.process_sort = 3;
    settings.process_descending = false;
    settings.process_quick_filter = 2;
    settings.process_own_user = true;
    let decoded: UserSettings =
        serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
    assert_eq!(settings, decoded);
}

#[test]
fn selected_command_lines_are_not_added_to_existing_snapshot_exports() {
    use voltix::monitoring::{extended::Extended, processes::ProcessDetails};
    let data = Extended {
        process_details: Some(ProcessDetails {
            process: Process::default(),
            executable: Some("/private/executable".into()),
            command: Some("private-command-argument".into()),
            command_truncated: false,
        }),
        ..Default::default()
    };
    let json = serde_json::to_string(&data).unwrap();
    assert!(!json.contains("process_details") && !json.contains("private-command-argument"));
}
