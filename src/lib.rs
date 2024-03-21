pub mod format;
pub mod time;
pub mod writer;

use std::path::PathBuf;

use tracing_core::LevelFilter;
use tracing_subscriber::fmt::SubscriberBuilder;

/// Config for formatters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
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
        /// See [`tracing_appender::rolling::Builder::max_log_files`].
        limit: Option<usize>,
        /// See [`tracing_appender::rolling::Builder::filename_prefix`].
        prefix: Option<String>,
        /// See [`tracing_appender::rolling::Builder::filename_suffix`].
        suffix: Option<String>,
        /// See [`tracing_appender::rolling::Builder::rotation`].
        rotation: Rotation,
        /// Wrap the writer in a [`tracing_appender::non_blocking::NonBlocking`].
        non_blocking: Option<NonBlocking>,
    },
}

/// How often to rotate the [`tracing_appender::rolling::RollingFileAppender`].
///
/// See [`tracing_appender::rolling::Rotation`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rotation {
    Minutely,
    Hourly,
    Daily,
    Never,
}

/// How the [`tracing_appender::non_blocking::NonBlocking`] should behave on a full queue.
///
/// See [`tracing_appender::non_blocking::NonBlockingBuilder::lossy`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BackpressureBehaviour {
    Drop,
    Block,
}

/// How to treat a newly created log file in [`Writer::File`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileOpenBehaviour {
    Truncate,
    Append,
}

/// Configuration for [`tracing_appender::non_blocking::NonBlocking`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NonBlocking {
    /// See [`tracing_appender::non_blocking::NonBlockingBuilder::buffered_lines_limit`].
    pub buffer_length: Option<usize>,
    pub behaviour: Option<BackpressureBehaviour>,
}

pub fn new(
    format: Format,
    writer: Writer,
) -> SubscriberBuilder<
    format::FormatFields,
    format::FormatEvent,
    tracing_core::LevelFilter,
    writer::MakeWriter,
> {
    let (writer, guard) = writer::MakeWriter::new(writer).unwrap();
    tracing_subscriber::fmt()
        .fmt_fields(format::FormatFields::from(
            format.formatter.clone().unwrap_or_default(),
        ))
        .event_format(format::FormatEvent::from(format))
        .with_max_level(LevelFilter::TRACE)
        .with_writer(writer)
}
