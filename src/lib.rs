pub mod format;
pub mod time;
pub mod writer;

use std::path::PathBuf;

use writer::Guard;

/// Configuration for a totally dynamic subscriber.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Subscriber {
    pub format: Option<Format>,
    pub writer: Option<Writer>,
}

/// A totally dynamically configured subscriber.
pub type SubscriberBuilder<
    N = format::FormatFields,
    E = format::FormatEvent,
    F = tracing_core::LevelFilter,
    W = writer::MakeWriter,
> = tracing_subscriber::fmt::SubscriberBuilder<N, E, F, W>;

impl Subscriber {
    pub fn builder(self) -> Result<(SubscriberBuilder, Guard), writer::Error> {
        let Self { format, writer } = self;
        let writer = writer.unwrap_or_default();
        let format = format.unwrap_or_default();
        let (writer, guard) = writer::MakeWriter::new(writer)?;
        let builder = tracing_subscriber::fmt()
            .fmt_fields(format::FormatFields::from(
                format.formatter.clone().unwrap_or_default(),
            ))
            .event_format(format::FormatEvent::from(format))
            .with_writer(writer);
        Ok((builder, guard))
    }
}

/// Config for formatters.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Format {
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_ansi`].
    pub ansi: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_target`].
    pub target: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_level`].
    pub level: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_thread_ids`].
    pub thread_ids: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_thread_names`].
    pub thread_names: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_file`].
    pub file: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_line_number`].
    pub line_number: Option<bool>,
    /// Specific output formats.
    pub formatter: Option<Formatter>,
    /// What timing information to include.
    pub timer: Option<Timer>,
}

/// The specific output format.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Formatter {
    /// See [`tracing_subscriber::fmt::format::Full`].
    #[default]
    Full,
    /// See [`tracing_subscriber::fmt::format::Compact`].
    Compact,
    /// See [`tracing_subscriber::fmt::format::Pretty`].
    Pretty,
    /// See [`tracing_subscriber::fmt::format::Json`].
    Json {
        /// See [`tracing_subscriber::fmt::format::Json::flatten_event`].
        flatten_event: Option<bool>,
        /// See [`tracing_subscriber::fmt::format::Json::with_current_span`].
        current_span: Option<bool>,
        /// See [`tracing_subscriber::fmt::format::Json::with_span_list`].
        span_list: Option<bool>,
    },
}

/// Which timer implementation to use.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Timer {
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::without_time`].
    None,
    /// See [`tracing_subscriber::fmt::time::ChronoLocal`].
    Local(Option<String>),
    /// See [`tracing_subscriber::fmt::time::ChronoUtc`].
    Utc(Option<String>),
    /// See [`tracing_subscriber::fmt::time::SystemTime`].
    #[default]
    System,
    /// See [`tracing_subscriber::fmt::time::Uptime`].
    Uptime,
}

/// Which writer to use.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Writer {
    /// No writer.
    Null,
    /// Use [`io::stdout`](std::io::stdout).
    #[default]
    Stdout,
    /// Use [`io::stderr`](std::io::stderr).
    Stderr,
    /// Write to a [`File`](std::fs::File).
    File {
        path: PathBuf,
        behaviour: FileOpenBehaviour,
        /// Wrap the writer in a [`tracing_appender::non_blocking::NonBlocking`].
        non_blocking: Option<NonBlocking>,
    },
    /// Use a [`tracing_appender::rolling::RollingFileAppender`].
    Rolling {
        directory: PathBuf,
        rolling: Rolling,
        /// Wrap the writer in a [`tracing_appender::non_blocking::NonBlocking`].
        non_blocking: Option<NonBlocking>,
    },
}

/// How often to rotate the [`tracing_appender::rolling::RollingFileAppender`].
///
/// See [`tracing_appender::rolling::Rotation`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Rotation {
    Minutely,
    Hourly,
    Daily,
    #[default]
    Never,
}
/// Config for [`tracing_appender::rolling::RollingFileAppender`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Rolling {
    /// See [`tracing_appender::rolling::Builder::max_log_files`].
    limit: Option<usize>,
    /// See [`tracing_appender::rolling::Builder::filename_prefix`].
    prefix: Option<String>,
    /// See [`tracing_appender::rolling::Builder::filename_suffix`].
    suffix: Option<String>,
    /// See [`tracing_appender::rolling::Builder::rotation`].
    rotation: Rotation,
}

/// How the [`tracing_appender::non_blocking::NonBlocking`] should behave on a full queue.
///
/// See [`tracing_appender::non_blocking::NonBlockingBuilder::lossy`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BackpressureBehaviour {
    Drop,
    Block,
}

/// How to treat a newly created log file in [`Writer::File`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileOpenBehaviour {
    Truncate,
    Append,
}

/// Configuration for [`tracing_appender::non_blocking::NonBlocking`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct NonBlocking {
    /// See [`tracing_appender::non_blocking::NonBlockingBuilder::buffered_lines_limit`].
    pub buffer_length: Option<usize>,
    pub behaviour: Option<BackpressureBehaviour>,
}
