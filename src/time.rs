use std::time::Instant;

use tracing_subscriber::fmt::{
    format::Writer,
    time::{ChronoLocal, ChronoUtc, SystemTime, Uptime},
};

pub enum Timer {
    None,
    Local(Option<String>),
    Utc(Option<String>),
    System,
    Uptime,
}

pub struct FormatTime(FormatTimeInner);

impl From<Timer> for FormatTime {
    fn from(value: Timer) -> Self {
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

impl From<Timer> for FormatTimeInner {
    fn from(value: Timer) -> Self {
        match value {
            Timer::None => Self::None(()),
            Timer::Local(it) => Self::Local(match it {
                Some(it) if it == "%+" => ChronoLocal::rfc_3339(),
                None => ChronoLocal::rfc_3339(),
                Some(it) => ChronoLocal::new(it),
            }),
            Timer::Utc(it) => Self::Utc(match it {
                Some(it) if it == "%+" => ChronoUtc::rfc_3339(),
                None => ChronoUtc::rfc_3339(),
                Some(it) => ChronoUtc::new(it),
            }),
            Timer::System => Self::System(SystemTime),
            Timer::Uptime => Self::Uptime(Uptime::from(Instant::now())),
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
