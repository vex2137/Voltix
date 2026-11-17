//! Processes presentation and filtering. All procfs I/O stays on the sampler thread.
use crate::{
    app::{AppWindow, ControlRow, ControlSummary, I18n, ProcessItem},
    localization::{LocaleCode, Localizer},
    models::telemetry::{CpuUtilizationScope, TelemetrySnapshot},
    monitoring::processes::{Process, ProcessKey},
};
use slint::ComponentHandle;

pub fn localize(window: &AppWindow, loc: &Localizer) {
    let i = window.global::<I18n>();
    i.set_proc_process_table(loc.tr("proc_process_table").into());
    i.set_proc_name(loc.tr("proc_name").into());
    i.set_proc_user(loc.tr("proc_user").into());
    i.set_proc_state(loc.tr("proc_state").into());
    i.set_proc_threads(loc.tr("proc_threads").into());
    i.set_proc_count(loc.tr("proc_count").into());
    i.set_proc_top_cpu(loc.tr("proc_top_cpu").into());
    i.set_proc_top_ram(loc.tr("proc_top_ram").into());
    i.set_proc_all(loc.tr("proc_all").into());
    i.set_proc_high_cpu(loc.tr("proc_high_cpu").into());
    i.set_proc_high_ram(loc.tr("proc_high_ram").into());
    i.set_proc_own(loc.tr("proc_own").into());
    i.set_proc_sort_name(loc.tr("proc_sort_name").into());
    i.set_proc_ascending(loc.tr("proc_ascending").into());
    i.set_proc_descending(loc.tr("proc_descending").into());
    i.set_proc_details(loc.tr("proc_details").into());
    i.set_proc_close(loc.tr("proc_close").into());
    i.set_proc_parent(loc.tr("proc_parent").into());
    i.set_proc_executable(loc.tr("proc_executable").into());
    i.set_proc_command(loc.tr("proc_command").into());
    i.set_proc_start(loc.tr("proc_start").into());
    i.set_proc_identity(loc.tr("proc_identity").into());
    i.set_proc_gone(loc.tr("proc_gone").into());
    i.set_proc_pending(loc.tr("proc_pending").into());
    i.set_proc_no_match(loc.tr("proc_no_match").into());
    i.set_proc_note(loc.tr("proc_note").into());
    i.set_proc_sampled(loc.tr("proc_sampled").into());
    i.set_proc_shown(loc.tr("proc_shown").into());
    i.set_proc_limit(loc.tr("proc_limit").into());
    i.set_proc_tasks_note(loc.tr("proc_tasks_note").into());
    i.set_proc_command_cut(loc.tr("proc_command_cut").into());
    i.set_proc_running(loc.tr("proc_running").into());
    i.set_proc_sleeping(loc.tr("proc_sleeping").into());
    i.set_proc_disk_sleep(loc.tr("proc_disk_sleep").into());
    i.set_proc_stopped(loc.tr("proc_stopped").into());
    i.set_proc_zombie(loc.tr("proc_zombie").into());
    i.set_proc_idle(loc.tr("proc_idle").into());
    i.set_proc_dead(loc.tr("proc_dead").into());
    i.set_proc_unknown(loc.tr("proc_unknown").into());
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub query: String,
    pub sort: i32,
    pub descending: bool,
    pub quick: i32,
    pub own_uid: Option<u32>,
}
impl Default for Filter {
    fn default() -> Self {
        Self {
            query: String::new(),
            sort: 0,
            descending: true,
            quick: 0,
            own_uid: None,
        }
    }
}
/// Unknown CPU samples sort last in either direction; equal values use PID/start
/// as a deterministic tiebreaker, so refreshes do not randomly shuffle ties.
pub fn filtered<'a>(processes: &'a [Process], f: &Filter) -> Vec<&'a Process> {
    let query = f.query.trim().to_lowercase();
    let mut rows: Vec<_> = processes
        .iter()
        .filter(|p| {
            (query.is_empty()
                || p.name.to_lowercase().contains(&query)
                || p.pid.to_string().contains(&query))
                && f.own_uid.is_none_or(|uid| p.uid == Some(uid))
                && match f.quick {
                    1 => p.cpu_percent.is_some_and(|v| v >= 10.),
                    2 => p.memory >= 256 * 1024 * 1024,
                    _ => true,
                }
        })
        .collect();
    if f.sort == 3 {
        if f.descending {
            rows.sort_by_cached_key(|p| {
                (
                    std::cmp::Reverse(p.name.to_lowercase()),
                    p.pid,
                    p.start_ticks,
                )
            });
        } else {
            rows.sort_by_cached_key(|p| (p.name.to_lowercase(), p.pid, p.start_ticks));
        }
        return rows;
    }
    rows.sort_by(|a, b| {
        let ordering = match f.sort {
            1 => a.memory.cmp(&b.memory),
            2 => a.pid.cmp(&b.pid),
            _ => match (a.cpu_percent, b.cpu_percent) {
                (Some(a), Some(b)) => a.total_cmp(&b),
                (None, Some(_)) => return std::cmp::Ordering::Greater,
                (Some(_), None) => return std::cmp::Ordering::Less,
                (None, None) => std::cmp::Ordering::Equal,
            },
        };
        (if f.descending {
            ordering.reverse()
        } else {
            ordering
        })
        .then(a.pid.cmp(&b.pid))
        .then(a.start_ticks.cmp(&b.start_ticks))
    });
    rows
}
#[derive(Default)]
pub struct ProcessUiCache(Option<(u64, Filter, String, LocaleCode)>);

pub fn state_key(state: char) -> &'static str {
    match state {
        'R' => "proc_running",
        'S' => "proc_sleeping",
        'D' => "proc_disk_sleep",
        'T' | 't' => "proc_stopped",
        'Z' => "proc_zombie",
        'I' => "proc_idle",
        'X' | 'x' => "proc_dead",
        _ => "proc_unknown",
    }
}
fn optional<T: ToString>(value: Option<T>, loc: &Localizer) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| loc.tr("status_unavailable").into())
}
fn cpu(p: &Process, loc: &Localizer) -> String {
    optional(p.cpu_percent.map(|v| format!("{v:.1}%")), loc)
}
fn mib(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_048_576.)
}
fn row(label: &str, value: String, loc: &Localizer) -> ControlRow {
    ControlRow {
        label: loc.tr(label).into(),
        value: value.into(),
        detail: "".into(),
    }
}
fn summary(label: &str, value: String, detail: String, loc: &Localizer) -> ControlSummary {
    ControlSummary {
        label: loc.tr(label).into(),
        value: value.into(),
        detail: detail.into(),
        percent: 0.,
        status: 0,
    }
}

pub fn render(
    window: &AppWindow,
    snap: &TelemetrySnapshot,
    loc: &Localizer,
    cache: &mut ProcessUiCache,
) {
    if window.get_current_page() != 10 {
        return;
    }
    let e = &snap.extended;
    let cpu_value = (snap.cpu.utilization_scope == CpuUtilizationScope::SystemWide)
        .then(|| snap.cpu.overall_utilization.copied())
        .flatten();
    let top_cpu = e
        .processes
        .iter()
        .filter(|p| p.cpu_percent.is_some())
        .max_by(|a, b| a.cpu_percent.unwrap().total_cmp(&b.cpu_percent.unwrap()));
    let top_ram = e.processes.iter().max_by_key(|p| p.memory);
    let unavailable = || loc.tr("status_unavailable").to_string();
    crate::app::update_vec_model(
        window.get_proc_summaries(),
        vec![
            summary(
                "proc_count",
                optional(e.process_count, loc),
                format!("{} {}", e.processes.len(), loc.tr("proc_sampled")),
                loc,
            ),
            summary(
                "proc_threads",
                optional(e.thread_count, loc),
                String::new(),
                loc,
            ),
            summary(
                "CPU",
                optional(cpu_value.map(|v| format!("{v:.1}%")), loc),
                String::new(),
                loc,
            ),
            summary(
                "cc_memory",
                optional(
                    snap.memory
                        .utilization_percent
                        .copied()
                        .map(|v| format!("{v:.1}%")),
                    loc,
                ),
                String::new(),
                loc,
            ),
            summary(
                "proc_top_cpu",
                top_cpu.map(|p| cpu(p, loc)).unwrap_or_else(unavailable),
                top_cpu.map(|p| p.name.clone()).unwrap_or_default(),
                loc,
            ),
            summary(
                "proc_top_ram",
                top_ram
                    .map(|p| format!("{} MiB", mib(p.memory)))
                    .unwrap_or_else(unavailable),
                top_ram.map(|p| p.name.clone()).unwrap_or_default(),
                loc,
            ),
        ],
    );
    let f = Filter {
        query: window.get_cc_filter().to_string(),
        sort: window.get_cc_sort(),
        descending: window.get_proc_descending(),
        quick: window.get_proc_quick(),
        own_uid: window
            .get_proc_own()
            .then_some(e.process_current_uid)
            .flatten(),
    };
    let selected = window.get_proc_selected().to_string();
    let stamp = (
        e.process_generation,
        f.clone(),
        selected.clone(),
        loc.locale(),
    );
    window.set_cc_processes_available(e.processes_available);
    if cache.0.as_ref() == Some(&stamp) {
        return;
    }
    cache.0 = Some(stamp);
    let processes = filtered(&e.processes, &f);
    window.set_proc_count_note(
        format!(
            "{} {} · {} {}{}",
            processes.len(),
            loc.tr("proc_shown"),
            e.processes.len(),
            loc.tr("proc_sampled"),
            if e.process_truncated {
                format!(" · {}", loc.tr("proc_limit"))
            } else {
                String::new()
            }
        )
        .into(),
    );
    let selection_key = ProcessKey::decode(&selected);
    window.set_proc_selected_index(
        processes
            .iter()
            .position(|p| Some(p.key()) == selection_key)
            .map(|i| i as i32)
            .unwrap_or(-1),
    );
    let total = snap.memory.total_bytes.copied().filter(|v| *v > 0);
    let rows = processes
        .into_iter()
        .map(|p| ProcessItem {
            key: p.key().encode().into(),
            name: p.name.clone().into(),
            pid: p.pid.to_string().into(),
            cpu_text: cpu(p, loc).into(),
            memory_text: mib(p.memory).into(),
            user: if p.user.is_empty() {
                unavailable()
            } else {
                p.user.clone()
            }
            .into(),
            state: loc.tr(state_key(p.state)).into(),
            threads: optional(p.threads, loc).into(),
            cpu: p.cpu_percent.unwrap_or(0.).clamp(0., 100.) as f32,
            ram: total
                .map(|n| 100. * p.memory as f64 / n as f64)
                .unwrap_or(0.)
                .clamp(0., 100.) as f32,
            hot: p.cpu_percent.is_some_and(|v| v >= 50.),
            running: p.state == 'R',
        })
        .collect();
    // Keep the model and delegates. Logical selection uses lifetime keys, not row indexes.
    crate::app::update_vec_model(window.get_proc_rows(), rows);
    let key = selection_key;
    let current = key.and_then(|key| e.processes.iter().find(|p| p.key() == key));
    let details = e
        .process_details
        .as_ref()
        .filter(|d| Some(d.process.key()) == key);
    let p = details.map(|d| &d.process).or(current);
    let mut info = Vec::new();
    window.set_proc_detail_note(
        if p.is_none() {
            loc.tr("proc_gone")
        } else if details.is_none() {
            loc.tr("proc_pending")
        } else {
            loc.tr("proc_identity")
        }
        .into(),
    );
    if let Some(p) = p {
        let full_name = details
            .and_then(|d| d.executable.as_deref())
            .and_then(|s| std::path::Path::new(s).file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.name.clone());
        info.extend([
            row("proc_name", full_name, loc),
            row("PID", p.pid.to_string(), loc),
            row("proc_parent", p.ppid.to_string(), loc),
            row("CPU", cpu(p, loc), loc),
            row("RAM", format!("{} MiB", mib(p.memory)), loc),
            row(
                "proc_user",
                p.uid
                    .map(|uid| format!("{} ({uid})", p.user))
                    .unwrap_or_else(unavailable),
                loc,
            ),
            row(
                "proc_state",
                format!("{} ({})", loc.tr(state_key(p.state)), p.state),
                loc,
            ),
            row("proc_threads", optional(p.threads, loc), loc),
            row("proc_start", p.start_ticks.to_string(), loc),
        ]);
        if let Some(d) = details {
            info.push(row(
                "proc_executable",
                d.executable.clone().unwrap_or_else(unavailable),
                loc,
            ));
            info.push(ControlRow {
                label: loc.tr("proc_command").into(),
                value: d
                    .command
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .unwrap_or_else(unavailable)
                    .into(),
                detail: if d.command_truncated {
                    loc.tr("proc_command_cut")
                } else {
                    ""
                }
                .into(),
            });
        }
    }
    crate::app::update_vec_model(window.get_proc_detail_rows(), info);
}
