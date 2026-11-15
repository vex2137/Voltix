//! Read-only Linux process sampling. PID + start ticks identify a process lifetime.
use serde::Serialize;

pub const PROCESS_LIMIT: usize = 2048;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize)]
pub struct ProcessKey {
    pub pid: u32,
    pub start_ticks: u64,
}
impl ProcessKey {
    pub fn encode(self) -> String {
        format!("{}:{}", self.pid, self.start_ticks)
    }
    pub fn decode(value: &str) -> Option<Self> {
        let (pid, start) = value.split_once(':')?;
        let pid = pid.parse().ok()?;
        (pid != 0).then_some(Self {
            pid,
            start_ticks: start.parse().ok()?,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Process {
    pub pid: u32,
    pub start_ticks: u64,
    pub name: String,
    pub cpu_percent: Option<f64>,
    pub memory: u64,
    pub ppid: u32,
    pub uid: Option<u32>,
    pub user: String,
    pub state: char,
    pub threads: Option<u64>,
}
impl Process {
    pub fn key(&self) -> ProcessKey {
        ProcessKey {
            pid: self.pid,
            start_ticks: self.start_ticks,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ProcessDetails {
    pub process: Process,
    pub executable: Option<String>,
    pub command: Option<String>,
    pub command_truncated: bool,
}

pub fn parse_process(pid: u32, stat: &str, page_size: u64) -> Option<(Process, u64, u64)> {
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let name = stat.get(open + 1..close)?;
    let mut fields = stat.get(close + 1..)?.split_whitespace();
    let state = fields.next()?.chars().next()?;
    let ppid = fields.next()?.parse().ok()?;
    let user_ticks = fields.nth(9)?.parse::<u64>().ok()?; // field 14
    let ticks = user_ticks.checked_add(fields.next()?.parse().ok()?)?;
    let threads = fields.nth(4)?.parse().ok()?; // field 20
    let start = fields.nth(1)?.parse().ok()?; // field 22
    let rss = fields.nth(1)?.parse::<u64>().ok()?; // field 24
    Some((
        Process {
            pid,
            start_ticks: start,
            name: name
                .chars()
                .map(|c| if c.is_control() { '�' } else { c })
                .collect(),
            cpu_percent: None,
            memory: rss.checked_mul(page_size)?,
            ppid,
            state,
            threads: Some(threads),
            ..Process::default()
        },
        ticks,
        start,
    ))
}

/// Read the effective UID, rather than proc directory ownership (which can be
/// root for non-dumpable processes owned by another user).
pub fn parse_effective_uid(status: &str) -> Option<u32> {
    status.lines().find_map(|line| {
        line.strip_prefix("Uid:")?
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()
    })
}

/// Linux process CPU is a percentage of one logical core, and may exceed 100%.
pub fn cpu_delta(previous: u64, current: u64, seconds: f64, ticks_per_second: u64) -> Option<f64> {
    if ticks_per_second == 0 {
        return None;
    }
    super::extended::rate(previous, current, seconds).map(|n| n * 100. / ticks_per_second as f64)
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::{
        collections::HashMap,
        fs,
        io::Read,
        path::Path,
        sync::Arc,
        time::{Duration, Instant},
    };

    #[derive(Default)]
    pub struct ProcessMonitor {
        previous: HashMap<ProcessKey, u64>,
        at: Option<Instant>,
        users: HashMap<u32, String>,
        users_at: Option<Instant>,
        generation: u64,
    }
    pub struct ProcessSample {
        pub rows: Arc<Vec<Process>>,
        pub count: Option<u64>,
        pub threads: Option<u64>,
        pub current_uid: u32,
        pub available: bool,
        pub generation: u64,
        pub truncated: bool,
        pub details: Option<ProcessDetails>,
    }
    impl ProcessMonitor {
        pub fn reset(&mut self) {
            self.previous.clear();
            self.at = None;
        }
        pub fn sample(&mut self, now: Instant, selected: Option<ProcessKey>) -> ProcessSample {
            self.generation = self.generation.wrapping_add(1);
            if self
                .users_at
                .is_none_or(|at| now.duration_since(at) >= Duration::from_secs(60))
            {
                self.users = fs::read_to_string("/etc/passwd")
                    .unwrap_or_default()
                    .lines()
                    .filter_map(|s| {
                        let mut f = s.split(':');
                        let name = f.next()?;
                        f.next()?;
                        Some((f.next()?.parse().ok()?, name.to_owned()))
                    })
                    .collect();
                self.users_at = Some(now);
            }
            let dt = self.at.map(|at| now.duration_since(at).as_secs_f64());
            self.at = Some(now);
            let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
            let pages = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
            let entries = fs::read_dir("/proc");
            let available = entries.is_ok() && ticks > 0 && pages > 0;
            let mut rows = Vec::with_capacity(self.previous.len());
            let mut count = 0;
            let mut attempted = 0;
            let mut next = HashMap::with_capacity(self.previous.len());
            for entry in entries.into_iter().flatten().filter_map(Result::ok) {
                let Some(pid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|s| s.parse::<u32>().ok())
                else {
                    continue;
                };
                count += 1;
                if !available || attempted >= PROCESS_LIMIT {
                    continue;
                }
                attempted += 1;
                let path = entry.path();
                let Some((mut p, current, _)) = fs::read_to_string(path.join("stat"))
                    .ok()
                    .and_then(|s| parse_process(pid, &s, pages as u64))
                else {
                    continue;
                };
                p.uid = fs::read_to_string(path.join("status"))
                    .ok()
                    .and_then(|s| parse_effective_uid(&s));
                p.user = self.user(p.uid);
                p.cpu_percent = dt.and_then(|dt| {
                    self.previous
                        .get(&p.key())
                        .and_then(|old| cpu_delta(*old, current, dt, ticks as u64))
                });
                next.insert(p.key(), current);
                rows.push(p);
            }
            self.previous = next;
            // /proc/loadavg reports all existing kernel scheduling entities (threads).
            let threads = fs::read_to_string("/proc/loadavg")
                .ok()
                .and_then(|s| s.split_whitespace().nth(3)?.split_once('/')?.1.parse().ok());
            let details = selected.and_then(|key| self.details(key, pages.max(1) as u64, &rows));
            ProcessSample {
                rows: Arc::new(rows),
                count: available.then_some(count),
                threads,
                current_uid: unsafe { libc::geteuid() },
                available,
                generation: self.generation,
                truncated: count > PROCESS_LIMIT as u64,
                details,
            }
        }
        fn user(&self, uid: Option<u32>) -> String {
            uid.map(|uid| {
                self.users
                    .get(&uid)
                    .cloned()
                    .unwrap_or_else(|| uid.to_string())
            })
            .unwrap_or_default()
        }
        fn details(&self, key: ProcessKey, pages: u64, rows: &[Process]) -> Option<ProcessDetails> {
            let root = std::path::PathBuf::from(format!("/proc/{}", key.pid));
            let mut p = read_key(&root, pages, key)?;
            p.uid = fs::read_to_string(root.join("status"))
                .ok()
                .and_then(|s| parse_effective_uid(&s));
            p.user = self.user(p.uid);
            if let Some(sample) = rows.iter().find(|p| p.key() == key) {
                p.cpu_percent = sample.cpu_percent;
            }
            let executable = fs::read_link(root.join("exe"))
                .ok()
                .map(|p| p.to_string_lossy().into_owned());
            let mut bytes = Vec::new();
            let command = fs::File::open(root.join("cmdline")).ok().and_then(|f| {
                f.take(16_385).read_to_end(&mut bytes).ok()?;
                Some(
                    String::from_utf8_lossy(&bytes[..bytes.len().min(16_384)])
                        .split('\0')
                        .filter(|s| !s.is_empty())
                        .map(|s| format!("{s:?}"))
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            });
            // A recycled PID must never receive details or CPU from a different lifetime.
            read_key(&root, pages, key)?;
            Some(ProcessDetails {
                process: p,
                executable,
                command,
                command_truncated: bytes.len() > 16_384,
            })
        }
    }
    fn read_key(root: &Path, pages: u64, key: ProcessKey) -> Option<Process> {
        let p = parse_process(key.pid, &fs::read_to_string(root.join("stat")).ok()?, pages)?.0;
        (p.key() == key).then_some(p)
    }
}
#[cfg(target_os = "linux")]
pub use linux::ProcessMonitor;
