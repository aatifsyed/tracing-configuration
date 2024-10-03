//! Configuration-as-a-struct for [`tracing_subscriber::fmt::Subscriber`], to allow
//! for serializable, dynamic configuration, at the cost of compile-time specialization.

pub mod format;
pub mod time;
pub mod writer;

#[cfg(feature = "schemars")]
use schemars::JsonSchema;
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, path::PathBuf, str::FromStr};
use tracing_subscriber::EnvFilter;

use writer::Guard;

/// Configuration for a totally dynamic subscriber.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub struct Subscriber {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub format: Option<Format>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub writer: Option<Writer>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub filter: Option<Filter>,
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub struct Filter {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub regex: Option<bool>,
    pub directives: Vec<Directive>,
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub struct Directive(String);

impl Directive {
    fn directive(&self) -> tracing_subscriber::filter::Directive {
        self.0.parse().unwrap()
    }
}

impl FromStr for Directive {
    type Err = tracing_subscriber::filter::ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<tracing_subscriber::filter::Directive>() {
            Ok(_) => Ok(Self(String::from(s))),
            Err(e) => Err(e),
        }
    }
}

impl fmt::Display for Directive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Directive {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        stringify::deserialize(d)
    }
}
#[cfg(feature = "serde")]
impl Serialize for Directive {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        stringify::serialize(self, s)
    }
}

impl From<Filter> for EnvFilter {
    fn from(value: Filter) -> Self {
        let Filter { regex, directives } = value;
        directives.into_iter().fold(
            EnvFilter::builder()
                .with_regex(regex.unwrap_or_default())
                .parse_lossy(""),
            |acc, el| acc.add_directive(el.directive()),
        )
    }
}

#[cfg(feature = "serde")]
mod stringify {
    use std::{borrow::Cow, fmt, str::FromStr};

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn deserialize<'de, D: Deserializer<'de>, T>(d: D) -> Result<T, D::Error>
    where
        T: FromStr,
        T::Err: fmt::Display,
    {
        #[derive(Deserialize)]
        struct CowStr<'a>(#[serde(borrow)] Cow<'a, str>);
        let CowStr(s) = Deserialize::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
    pub fn serialize<S: Serializer, T>(t: &T, s: S) -> Result<S::Ok, S::Error>
    where
        T: fmt::Display,
    {
        s.collect_str(t)
    }
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Level {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl From<Level> for tracing_core::LevelFilter {
    fn from(value: Level) -> Self {
        match value {
            Level::Error => Self::ERROR,
            Level::Warn => Self::WARN,
            Level::Info => Self::INFO,
            Level::Debug => Self::DEBUG,
            Level::Trace => Self::TRACE,
        }
    }
}

/// A totally dynamically configured [`tracing_subscriber::fmt::SubscriberBuilder`].
pub type SubscriberBuilder<
    N = format::FormatFields,
    E = format::FormatEvent,
    F = EnvFilter,
    W = writer::MakeWriter,
> = tracing_subscriber::fmt::SubscriberBuilder<N, E, F, W>;

/// A totally dynamically configured [`tracing_subscriber::fmt::Layer`].
pub type Layer<S, N = format::FormatFields, E = format::FormatEvent, W = writer::MakeWriter> =
    tracing_subscriber::fmt::Layer<S, N, E, W>;

impl Subscriber {
    fn into_components(
        self,
        defer: bool,
    ) -> Result<
        (
            writer::MakeWriter,
            format::FormatFields,
            format::FormatEvent,
            EnvFilter,
            Guard,
        ),
        writer::Error,
    > {
        let Self {
            format,
            writer,
            filter,
        } = self;
        let format = format.unwrap_or_default();
        let writer = writer.unwrap_or_default();
        let (writer, guard) = match defer {
            true => writer::MakeWriter::try_new(writer)?,
            false => writer::MakeWriter::new(writer),
        };
        let fields = format::FormatFields::from(format.formatter.clone().unwrap_or_default());
        let event = format::FormatEvent::from(format);
        let filter = EnvFilter::from(filter.unwrap_or_default());
        Ok((writer, fields, event, filter, guard))
    }
    /// Create a new [`Layer`], and a [`Guard`] that handles e.g flushing [`NonBlocking`] IO.
    ///
    /// Errors when opening files or directories are deferred for the subscriber to handle (typically by logging).
    /// If you wish to handle them yourself, see [`Self::try_layer`].
    ///
    /// Note that filtering is ignored for layers.
    pub fn layer<S>(self) -> (Layer<S>, Guard)
    where
        S: tracing_core::Subscriber + for<'s> tracing_subscriber::registry::LookupSpan<'s>,
    {
        let (writer, fields, event, _filter, guard) = self
            .into_components(true)
            .expect("errors have been deferred");
        let layer = tracing_subscriber::fmt::layer()
            .fmt_fields(fields)
            .event_format(event)
            .with_writer(writer);
        (layer, guard)
    }
    /// Create a new [`Layer`], and a [`Guard`] that handles e.g flushing [`NonBlocking`] IO.
    ///
    /// Returns [`Err`] if e.g opening a log file fails.
    /// If you wish the subscriber to handle them (typically by logging), see [`Self::layer`].
    ///
    /// Note that filtering is ignored for layers.
    pub fn try_layer<S>(self) -> Result<(Layer<S>, Guard), writer::Error>
    where
        S: tracing_core::Subscriber + for<'s> tracing_subscriber::registry::LookupSpan<'s>,
    {
        let (writer, fields, event, _filter, guard) = self.into_components(false)?;
        let layer = tracing_subscriber::fmt::layer()
            .fmt_fields(fields)
            .event_format(event)
            .with_writer(writer);
        Ok((layer, guard))
    }
    /// Create a new [`SubscriberBuilder`], and a [`Guard`] that handles e.g flushing [`NonBlocking`] IO.
    ///
    /// Errors when opening files or directories are deferred for the subscriber to handle (typically by logging).
    /// If you wish to handle them yourself, see [`Self::try_builder`].
    pub fn builder(self) -> (SubscriberBuilder, Guard) {
        let (writer, fields, event, filter, guard) = self
            .into_components(true)
            .expect("errors have been deferred");
        let builder = tracing_subscriber::fmt()
            .fmt_fields(fields)
            .event_format(event)
            .with_writer(writer)
            .with_env_filter(filter);
        (builder, guard)
    }
    /// Create a new [`SubscriberBuilder`], and a [`Guard`] that handles e.g flushing [`NonBlocking`] IO.
    ///
    /// Returns [`Err`] if e.g opening a log file fails.
    /// If you wish the subscriber to handle them (typically by logging), see [`Self::builder`].
    pub fn try_builder(self) -> Result<(SubscriberBuilder, Guard), writer::Error> {
        let (writer, fields, event, filter, guard) = self.into_components(false)?;
        let builder = tracing_subscriber::fmt()
            .fmt_fields(fields)
            .event_format(event)
            .with_writer(writer)
            .with_env_filter(filter);
        Ok((builder, guard))
    }
}

/// Config for formatters.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub struct Format {
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_ansi`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub ansi: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_target`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub target: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_level`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub level: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_thread_ids`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub thread_ids: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_thread_names`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub thread_names: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_file`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub file: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_line_number`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub line_number: Option<bool>,
    /// Specific output formats.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub formatter: Option<Formatter>,
    /// What timing information to include.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub timer: Option<Timer>,
}

/// The specific output format.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
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

#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub struct Json {
    /// See [`tracing_subscriber::fmt::format::Json::flatten_event`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub flatten_event: Option<bool>,
    /// See [`tracing_subscriber::fmt::format::Json::with_current_span`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub current_span: Option<bool>,
    /// See [`tracing_subscriber::fmt::format::Json::with_span_list`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub span_list: Option<bool>,
}

/// Which timer implementation to use.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Timer {
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::without_time`].
    None,
    /// See [`tracing_subscriber::fmt::time::ChronoLocal`].
    Local(
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        Option<String>,
    ),
    /// See [`tracing_subscriber::fmt::time::ChronoUtc`].
    Utc(
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        Option<String>,
    ),
    /// See [`tracing_subscriber::fmt::time::SystemTime`].
    #[default]
    System,
    /// See [`tracing_subscriber::fmt::time::Uptime`].
    Uptime,
}

/// Which writer to use.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
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
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        non_blocking: Option<NonBlocking>,
    },
    /// Use a [`tracing_appender::rolling::RollingFileAppender`].
    Rolling {
        directory: PathBuf,
        rolling: Option<Rolling>,
        /// Wrap the writer in a [`tracing_appender::non_blocking::NonBlocking`].
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        non_blocking: Option<NonBlocking>,
    },
}

/// How often to rotate the [`tracing_appender::rolling::RollingFileAppender`].
///
/// See [`tracing_appender::rolling::Rotation`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Rotation {
    Minutely,
    Hourly,
    Daily,
    #[default]
    Never,
}
/// Config for [`tracing_appender::rolling::RollingFileAppender`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub struct Rolling {
    /// See [`tracing_appender::rolling::Builder::max_log_files`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub limit: Option<usize>,
    /// See [`tracing_appender::rolling::Builder::filename_prefix`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub prefix: Option<String>,
    /// See [`tracing_appender::rolling::Builder::filename_suffix`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub suffix: Option<String>,
    /// See [`tracing_appender::rolling::Builder::rotation`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub rotation: Option<Rotation>,
}

/// How the [`tracing_appender::non_blocking::NonBlocking`] should behave on a full queue.
///
/// See [`tracing_appender::non_blocking::NonBlockingBuilder::lossy`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum BackpressureBehaviour {
    Drop,
    Block,
}

/// How to treat a newly created log file in [`Writer::File`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum FileOpenBehaviour {
    Truncate,
    Append,
}

/// Configuration for [`tracing_appender::non_blocking::NonBlocking`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub struct NonBlocking {
    /// See [`tracing_appender::non_blocking::NonBlockingBuilder::buffered_lines_limit`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub buffer_length: Option<usize>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub behaviour: Option<BackpressureBehaviour>,
}

#[cfg(all(test, feature = "schemars"))]
#[test]
fn schema() {
    let s = serde_json::to_string_pretty(&schemars::schema_for!(Subscriber)).unwrap();
    expect_test::expect_file!["../snapshots/schema.json"].assert_eq(&s);
}
