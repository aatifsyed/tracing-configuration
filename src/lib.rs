//! Configuration-as-a-struct for [`tracing_subscriber::fmt::Subscriber`], to allow
//! for serializable, dynamic configuration, at the cost of compile-time specialization.

pub mod format;
pub mod time;
pub mod writer;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use writer::Guard;

/// Configuration for a totally dynamic subscriber.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Subscriber {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<Format>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writer: Option<Writer>,
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LevelFilter {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl From<LevelFilter> for tracing_core::LevelFilter {
    fn from(value: LevelFilter) -> Self {
        match value {
            LevelFilter::Error => Self::ERROR,
            LevelFilter::Warn => Self::WARN,
            LevelFilter::Info => Self::INFO,
            LevelFilter::Debug => Self::DEBUG,
            LevelFilter::Trace => Self::TRACE,
        }
    }
}

/// A totally dynamically configured [`tracing_subscriber::fmt::SubscriberBuilder`].
pub type SubscriberBuilder<
    N = format::FormatFields,
    E = format::FormatEvent,
    F = tracing_core::LevelFilter,
    W = writer::MakeWriter,
> = tracing_subscriber::fmt::SubscriberBuilder<N, E, F, W>;

/// A totally dynamically configured [`tracing_subscriber::fmt::Layer`].
pub type Layer<S, N = format::FormatFields, E = format::FormatEvent, W = writer::MakeWriter> =
    tracing_subscriber::fmt::Layer<S, N, E, W>;

impl Subscriber {
    fn into_components(
        self,
    ) -> Result<
        (
            writer::MakeWriter,
            format::FormatFields,
            format::FormatEvent,
            Guard,
        ),
        writer::Error,
    > {
        let Self { format, writer } = self;
        let format = format.unwrap_or_default();
        let writer = writer.unwrap_or_default();
        let (writer, guard) = writer::MakeWriter::new(writer)?;
        let fields = format::FormatFields::from(format.formatter.clone().unwrap_or_default());
        let event = format::FormatEvent::from(format);
        Ok((writer, fields, event, guard))
    }
    pub fn layer<S>(self) -> Result<(Layer<S>, Guard), writer::Error>
    where
        S: tracing_core::Subscriber + for<'s> tracing_subscriber::registry::LookupSpan<'s>,
    {
        let (writer, fields, event, guard) = self.into_components()?;
        let layer = tracing_subscriber::fmt::layer()
            .fmt_fields(fields)
            .event_format(event)
            .with_writer(writer);
        Ok((layer, guard))
    }
    pub fn builder(self) -> Result<(SubscriberBuilder, Guard), writer::Error> {
        let (writer, fields, event, guard) = self.into_components()?;
        let builder = tracing_subscriber::fmt()
            .fmt_fields(fields)
            .event_format(event)
            .with_writer(writer);
        Ok((builder, guard))
    }
}

/// Config for formatters.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Format {
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_ansi`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ansi: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_target`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_level`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_thread_ids`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_ids: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_thread_names`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_names: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_file`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_line_number`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_number: Option<bool>,
    /// Specific output formats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formatter: Option<Formatter>,
    /// What timing information to include.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer: Option<Timer>,
}

/// The specific output format.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Formatter {
    /// See [`tracing_subscriber::fmt::format::Full`].
    #[default]
    Full,
    /// See [`tracing_subscriber::fmt::format::Compact`].
    Compact,
    /// See [`tracing_subscriber::fmt::format::Pretty`].
    Pretty,
    /// See [`tracing_subscriber::fmt::format::Json`].
    Json(Option<Json>),
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Json {
    /// See [`tracing_subscriber::fmt::format::Json::flatten_event`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    flatten_event: Option<bool>,
    /// See [`tracing_subscriber::fmt::format::Json::with_current_span`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    current_span: Option<bool>,
    /// See [`tracing_subscriber::fmt::format::Json::with_span_list`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    span_list: Option<bool>,
}

/// Which timer implementation to use.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Timer {
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::without_time`].
    None,
    /// See [`tracing_subscriber::fmt::time::ChronoLocal`].
    Local(#[serde(default, skip_serializing_if = "Option::is_none")] Option<String>),
    /// See [`tracing_subscriber::fmt::time::ChronoUtc`].
    Utc(#[serde(default, skip_serializing_if = "Option::is_none")] Option<String>),
    /// See [`tracing_subscriber::fmt::time::SystemTime`].
    #[default]
    System,
    /// See [`tracing_subscriber::fmt::time::Uptime`].
    Uptime,
}

/// Which writer to use.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        non_blocking: Option<NonBlocking>,
    },
    /// Use a [`tracing_appender::rolling::RollingFileAppender`].
    Rolling {
        directory: PathBuf,
        rolling: Option<Rolling>,
        /// Wrap the writer in a [`tracing_appender::non_blocking::NonBlocking`].
        #[serde(default, skip_serializing_if = "Option::is_none")]
        non_blocking: Option<NonBlocking>,
    },
}

/// How often to rotate the [`tracing_appender::rolling::RollingFileAppender`].
///
/// See [`tracing_appender::rolling::Rotation`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rotation {
    Minutely,
    Hourly,
    Daily,
    #[default]
    Never,
}
/// Config for [`tracing_appender::rolling::RollingFileAppender`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Rolling {
    /// See [`tracing_appender::rolling::Builder::max_log_files`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    limit: Option<usize>,
    /// See [`tracing_appender::rolling::Builder::filename_prefix`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prefix: Option<String>,
    /// See [`tracing_appender::rolling::Builder::filename_suffix`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    suffix: Option<String>,
    /// See [`tracing_appender::rolling::Builder::rotation`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rotation: Option<Rotation>,
}

/// How the [`tracing_appender::non_blocking::NonBlocking`] should behave on a full queue.
///
/// See [`tracing_appender::non_blocking::NonBlockingBuilder::lossy`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackpressureBehaviour {
    Drop,
    Block,
}

/// How to treat a newly created log file in [`Writer::File`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileOpenBehaviour {
    Truncate,
    Append,
}

/// Configuration for [`tracing_appender::non_blocking::NonBlocking`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct NonBlocking {
    /// See [`tracing_appender::non_blocking::NonBlockingBuilder::buffered_lines_limit`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffer_length: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behaviour: Option<BackpressureBehaviour>,
}
