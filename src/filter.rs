pub enum LevelFilter {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<LevelFilter> for tracing_core::LevelFilter {
    fn from(value: LevelFilter) -> Self {
        match value {
            LevelFilter::Off => Self::OFF,
            LevelFilter::Error => Self::ERROR,
            LevelFilter::Warn => Self::WARN,
            LevelFilter::Info => Self::INFO,
            LevelFilter::Debug => Self::DEBUG,
            LevelFilter::Trace => Self::TRACE,
        }
    }
}

pub mod targets {
    use super::LevelFilter;

    pub struct Targets(pub Vec<(String, LevelFilter)>);

    impl From<Targets> for tracing_subscriber::filter::Targets {
        fn from(Targets(value): Targets) -> Self {
            Self::from_iter(
                value
                    .into_iter()
                    .map(|(target, level)| (target, tracing_core::LevelFilter::from(level))),
            )
        }
    }
}
