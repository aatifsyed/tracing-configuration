use std::time::Instant;

use tracing_subscriber::fmt::{
    format::Writer,
    time::{ChronoLocal, ChronoUtc, SystemTime, Uptime},
};

/// Implementor of [`tracing_subscriber::fmt::time::FormatTime`], constructed [`From`] [`Timer`](crate::Timer).
pub struct FormatTime(FormatTimeInner);

impl From<crate::Timer> for FormatTime {
    fn from(value: crate::Timer) -> Self {
        Self(value.into())
    }
}

impl tracing_subscriber::fmt::time::FormatTime for FormatTime {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        self.0.format_time(w)
    }
}

enum FormatTimeInner {
    None(()),
    Local(ChronoLocal),
    Utc(ChronoUtc),
    System(SystemTime),
    Uptime(Uptime),
}

impl From<crate::Timer> for FormatTimeInner {
    fn from(value: crate::Timer) -> Self {
        match value {
            crate::Timer::None => Self::None(()),
            crate::Timer::Local(it) => Self::Local(match it {
                Some(it) if it == "%+" => ChronoLocal::rfc_3339(),
                None => ChronoLocal::rfc_3339(),
                Some(it) => ChronoLocal::new(it),
            }),
            crate::Timer::Utc(it) => Self::Utc(match it {
                Some(it) if it == "%+" => ChronoUtc::rfc_3339(),
                None => ChronoUtc::rfc_3339(),
                Some(it) => ChronoUtc::new(it),
            }),
            crate::Timer::System => Self::System(SystemTime),
            crate::Timer::Uptime => Self::Uptime(Uptime::from(Instant::now())),
        }
    }
}

impl tracing_subscriber::fmt::time::FormatTime for FormatTimeInner {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        match self {
            Self::None(it) => it.format_time(w),
            Self::Local(it) => it.format_time(w),
            Self::Utc(it) => it.format_time(w),
            Self::System(it) => it.format_time(w),
            Self::Uptime(it) => it.format_time(w),
        }
    }
}
