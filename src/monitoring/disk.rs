use std::collections::BTreeMap;
#[cfg(all(unix, not(target_os = "android")))]
use std::ffi::CString;
#[cfg(not(target_os = "android"))]
use std::fs;

use crate::models::metrics::MetricValue;
use crate::models::telemetry::StorageDeviceTelemetry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountEntry {
    pub device: String,
    pub mount_point: String,
    pub filesystem: String,
}

const IGNORED_FILESYSTEMS: &[&str] = &[
    "proc",
    "sysfs",
    "tmpfs",
    "devtmpfs",
    "devpts",
    "cgroup",
    "cgroup2",
    "pstore",
    "bpf",
    "tracefs",
    "debugfs",
    "securityfs",
    "fusectl",
    "configfs",
    "efivarfs",
    "ramfs",
    "hugetlbfs",
    "mqueue",
    "autofs",
    "overlay",
    "nsfs",
    "rpc_pipefs",
    "squashfs",
    "binfmt_misc",
    "fuse.portal",
    "fuse.gvfsd-fuse",
];

#[derive(Default)]
pub struct DiskMonitor;

impl DiskMonitor {
    pub fn new() -> Self {
        Self
    }

    pub fn collect(&self) -> Vec<StorageDeviceTelemetry> {
        #[cfg(target_os = "android")]
        {
            crate::platform::android::collect_android_storage()
        }
        #[cfg(not(target_os = "android"))]
        {
            let Ok(mounts_raw) = fs::read_to_string("/proc/mounts") else {
                return Vec::new();
            };

            let mounts = parse_and_deduplicate_mounts(&mounts_raw);
            mounts
                .into_iter()
                .filter_map(|m| query_mount_stats(&m))
                .collect()
        }
    }
}

pub fn parse_and_deduplicate_mounts(content: &str) -> Vec<MountEntry> {
    // Keyed by backing device string (e.g. /dev/nvme0n1p2) to avoid double-counting
    // Btrfs subvolumes or bind mounts sharing the same underlying filesystem.
    let mut by_device: BTreeMap<String, MountEntry> = BTreeMap::new();

    for line in content.lines() {
        let mut parts = line.split_whitespace();
        let Some(device) = parts.next() else {
            continue;
        };
        let Some(raw_mount) = parts.next() else {
            continue;
        };
        let Some(fs_type) = parts.next() else {
            continue;
        };

        if IGNORED_FILESYSTEMS.contains(&fs_type) {
            continue;
        }
        if !device.starts_with('/') || device.starts_with("/dev/loop") {
            continue;
        }

        let mount_point = unescape_mount_path(raw_mount);
        let candidate = MountEntry {
            device: device.to_string(),
            mount_point,
            filesystem: fs_type.to_string(),
        };

        match by_device.get(&candidate.device) {
            Some(existing) => {
                // Always prefer "/" if present, otherwise prefer the shorter mount path.
                let replace = candidate.mount_point == "/"
                    || (existing.mount_point != "/"
                        && candidate.mount_point.len() < existing.mount_point.len());
                if replace {
                    by_device.insert(candidate.device.clone(), candidate);
                }
            }
            None => {
                by_device.insert(candidate.device.clone(), candidate);
            }
        }
    }

    let mut list: Vec<MountEntry> = by_device.into_values().collect();
    list.sort_by(|a, b| {
        if a.mount_point == "/" {
            std::cmp::Ordering::Less
        } else if b.mount_point == "/" {
            std::cmp::Ordering::Greater
        } else {
            a.mount_point.cmp(&b.mount_point)
        }
    });
    list
}

fn unescape_mount_path(raw: &str) -> String {
    raw.replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

#[cfg(all(unix, not(target_os = "android")))]
fn stat_err_metric<T>(is_permission_denied: bool, reason: &str) -> MetricValue<T> {
    if is_permission_denied {
        MetricValue::PermissionError(reason.to_string())
    } else {
        MetricValue::Unavailable(reason.to_string())
    }
}

#[cfg(all(unix, not(target_os = "android")))]
fn query_mount_stats(entry: &MountEntry) -> Option<StorageDeviceTelemetry> {
    let c_path = CString::new(entry.mount_point.as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };

    if rc != 0 {
        let err = std::io::Error::last_os_error();
        let is_perm = err.kind() == std::io::ErrorKind::PermissionDenied;
        let reason = err.to_string();
        return Some(StorageDeviceTelemetry {
            device: entry.device.clone(),
            mount_point: entry.mount_point.clone(),
            filesystem: entry.filesystem.clone(),
            total_bytes: stat_err_metric(is_perm, &reason),
            used_bytes: stat_err_metric(is_perm, &reason),
            available_bytes: stat_err_metric(is_perm, &reason),
            utilization_percent: stat_err_metric(is_perm, &reason),
        });
    }

    let block_size = if stat.f_frsize > 0 {
        stat.f_frsize as u64
    } else {
        stat.f_bsize as u64
    };

    let total = (stat.f_blocks as u64).saturating_mul(block_size);
    if total == 0 {
        return None;
    }
    let free_root = (stat.f_bfree as u64).saturating_mul(block_size);
    let available = (stat.f_bavail as u64).saturating_mul(block_size);
    let used = total.saturating_sub(free_root);
    let pct = ((used as f64) / (total as f64)) * 100.0;

    Some(StorageDeviceTelemetry {
        device: entry.device.clone(),
        mount_point: entry.mount_point.clone(),
        filesystem: entry.filesystem.clone(),
        total_bytes: MetricValue::Available(total),
        used_bytes: MetricValue::Available(used),
        available_bytes: MetricValue::Available(available),
        utilization_percent: MetricValue::Available(pct.clamp(0.0, 100.0)),
    })
}

#[cfg(not(any(unix, target_os = "android")))]
fn query_mount_stats(entry: &MountEntry) -> Option<StorageDeviceTelemetry> {
    Some(StorageDeviceTelemetry {
        device: entry.device.clone(),
        mount_point: entry.mount_point.clone(),
        filesystem: entry.filesystem.clone(),
        total_bytes: MetricValue::Unsupported("Non-Unix statvfs unsupported".into()),
        used_bytes: MetricValue::Unsupported("Non-Unix statvfs unsupported".into()),
        available_bytes: MetricValue::Unsupported("Non-Unix statvfs unsupported".into()),
        utilization_percent: MetricValue::Unsupported("Non-Unix statvfs unsupported".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_btrfs_subvolumes_and_ignores_virtual_fs() {
        let mounts = "\
proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
tmpfs /run tmpfs rw,nosuid,nodev 0 0
/dev/nvme0n1p2 /home btrfs rw,subvol=/@home 0 0
/dev/nvme0n1p2 / btrfs rw,subvol=/@ 0 0
/dev/nvme0n1p2 /var/log btrfs rw,subvol=/@log 0 0
/dev/nvme0n1p1 /boot vfat rw 0 0
/dev/loop0 /var/lib/snapd/snap/core/123 squashfs ro 0 0
";
        let list = parse_and_deduplicate_mounts(mounts);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].mount_point, "/");
        assert_eq!(list[0].device, "/dev/nvme0n1p2");
        assert_eq!(list[1].mount_point, "/boot");
    }
}
