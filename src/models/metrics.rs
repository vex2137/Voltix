use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricStatus {
    Available,
    Unavailable,
    Unsupported,
    PermissionError,
}

impl MetricStatus {
    pub fn as_code(self) -> i32 {
        match self {
            Self::Available => 0,
            Self::Unavailable => 1,
            Self::Unsupported => 2,
            Self::PermissionError => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MetricValue<T> {
    Available(T),
    Unavailable(String),
    Unsupported(String),
    PermissionError(String),
}

impl<T> MetricValue<T> {
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available(_))
    }

    pub fn status(&self) -> MetricStatus {
        match self {
            Self::Available(_) => MetricStatus::Available,
            Self::Unavailable(_) => MetricStatus::Unavailable,
            Self::Unsupported(_) => MetricStatus::Unsupported,
            Self::PermissionError(_) => MetricStatus::PermissionError,
        }
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Available(v) => Some(v),
            _ => None,
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Available(_) => None,
            Self::Unavailable(r) | Self::Unsupported(r) | Self::PermissionError(r) => {
                Some(r.as_str())
            }
        }
    }

    pub fn map<U, F: FnOnce(&T) -> U>(&self, f: F) -> MetricValue<U> {
        match self {
            Self::Available(v) => MetricValue::Available(f(v)),
            Self::Unavailable(r) => MetricValue::Unavailable(r.clone()),
            Self::Unsupported(r) => MetricValue::Unsupported(r.clone()),
            Self::PermissionError(r) => MetricValue::PermissionError(r.clone()),
        }
    }
}

impl<T: Copy> MetricValue<T> {
    pub fn copied(&self) -> Option<T> {
        match self {
            Self::Available(v) => Some(*v),
            _ => None,
        }
    }
}

impl<T: fmt::Display> fmt::Display for MetricValue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Available(v) => write!(f, "{v}"),
            Self::Unavailable(r) => write!(f, "Unavailable ({r})"),
            Self::Unsupported(r) => write!(f, "Unsupported ({r})"),
            Self::PermissionError(r) => write!(f, "Permission error ({r})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_value_distinguishes_all_states() {
        let avail = MetricValue::Available(42.5_f64);
        assert!(avail.is_available());
        assert_eq!(avail.status(), MetricStatus::Available);
        assert_eq!(avail.copied(), Some(42.5));
        assert_eq!(avail.reason(), None);

        let unavail: MetricValue<f64> = MetricValue::Unavailable("sensor offline".into());
        assert!(!unavail.is_available());
        assert_eq!(unavail.status(), MetricStatus::Unavailable);
        assert_eq!(unavail.copied(), None);
        assert_eq!(unavail.reason(), Some("sensor offline"));

        let unsup: MetricValue<f64> = MetricValue::Unsupported("no hardware counter".into());
        assert_eq!(unsup.status(), MetricStatus::Unsupported);
        assert_eq!(unsup.status().as_code(), 2);

        let perm: MetricValue<f64> = MetricValue::PermissionError("EACCES".into());
        assert_eq!(perm.status(), MetricStatus::PermissionError);
        assert_eq!(perm.status().as_code(), 3);
    }

    #[test]
    fn metric_value_map_preserves_unavailable_status() {
        let unavail: MetricValue<u64> = MetricValue::Unsupported("not exposed".into());
        let mapped = unavail.map(|v| v * 2);
        assert_eq!(mapped, MetricValue::Unsupported("not exposed".into()));
    }
}
