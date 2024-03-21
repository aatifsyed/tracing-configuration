mod format;
mod time;
mod writer;

use std::path::PathBuf;

use tracing_core::LevelFilter;
use tracing_subscriber::fmt::SubscriberBuilder;

/// Config for formatters.
pub struct Format {
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_ansi`].
    pub ansi: bool,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_target`].
    pub target: bool,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_level`].
    pub level: bool,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_thread_ids`].
    pub thread_ids: bool,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_thread_names`].
    pub thread_names: bool,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_file`].
    pub file: bool,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_line_number`].
    pub line_number: bool,
    /// Specific output formats.
    pub formatter: Formatter,
    /// What timing information to include.
    pub timer: Timer,
}

/// The specific output format.
pub enum Formatter {
    /// See [`tracing_subscriber::fmt::format::Full`].
    Full,
    /// See [`tracing_subscriber::fmt::format::Compact`].
    Compact,
    /// See [`tracing_subscriber::fmt::format::Pretty`].
    Pretty,
    /// See [`tracing_subscriber::fmt::format::Json`].
    Json {
        /// See [`tracing_subscriber::fmt::format::Json::flatten_event`].
        flatten_event: bool,
        /// See [`tracing_subscriber::fmt::format::Json::with_current_span`].
        current_span: bool,
        /// See [`tracing_subscriber::fmt::format::Json::with_span_list`].
        span_list: bool,
    },
}

/// Which timer implementation to use.
pub enum Timer {
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::without_time`].
    None,
    /// See [`tracing_subscriber::fmt::time::ChronoLocal`].
    Local(Option<String>),
    /// See [`tracing_subscriber::fmt::time::ChronoUtc`].
    Utc(Option<String>),
    /// See [`tracing_subscriber::fmt::time::SystemTime`].
    System,
    /// See [`tracing_subscriber::fmt::time::Uptime`].
    Uptime,
}

/// Which writer to use.
pub enum Writer {
    /// No writer.
    Null,
    /// Use [`io::stdout`](std::io::stdout).
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
pub enum Rotation {
    Minutely,
    Hourly,
    Daily,
    Never,
}

/// How the [`tracing_appender::non_blocking::NonBlocking`] should behave on a full queue.
///
/// See [`tracing_appender::non_blocking::NonBlockingBuilder::lossy`].
pub enum BackpressureBehaviour {
    Drop,
    Block,
}

/// How to treat a newly created log file in [`Writer::File`].
pub enum FileOpenBehaviour {
    Truncate,
    Append,
}

/// Configuration for [`tracing_appender::non_blocking::NonBlocking`].
pub struct NonBlocking {
    /// See [`tracing_appender::non_blocking::NonBlockingBuilder::buffered_lines_limit`].
    pub buffer_length: Option<usize>,
    pub behaviour: Option<BackpressureBehaviour>,
}

pub fn new(
    format: Format,
    writer: Writer,
) -> SubscriberBuilder<
    tracing_subscriber::fmt::format::DefaultFields,
    format::FormatEvent,
    tracing_core::LevelFilter,
    writer::MakeWriter,
> {
    let (writer, guard) = writer::MakeWriter::new(writer).unwrap();
    tracing_subscriber::fmt()
        .event_format(format::FormatEvent::from(format))
        .with_max_level(LevelFilter::TRACE)
        .with_writer(writer)
}
