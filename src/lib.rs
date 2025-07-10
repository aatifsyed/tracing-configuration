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
use winnow::{
    combinator::{alt, preceded, rest},
    Parser as _,
};

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
    pub const PARSE_HELP: &str = "target[span{field=value}]=level";
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

impl From<Level> for EnvFilter {
    fn from(value: Level) -> Self {
        Self::new(value.as_str())
    }
}

impl From<Level> for Directive {
    fn from(value: Level) -> Self {
        value.as_str().parse().unwrap()
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

#[derive(Debug)]
pub struct ParseError(&'static str);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("expected {}", self.0))
    }
}

impl std::error::Error for ParseError {}

macro_rules! strum {
    (
        $(#[$enum_meta:meta])*
        $vis:vis enum $enum_name:ident $parse_help:literal {
            $(
                $(#[$variant_meta:meta])*
                $variant_name:ident = $string:literal
            ),* $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        $vis enum $enum_name {
            $(
                $(#[$variant_meta])*
                $variant_name,
            )*
        }
        impl $enum_name {
            pub const PARSE_HELP: &str = $parse_help;
            pub const fn as_str(&self) -> &'static str {
                match *self {
                    $(
                        Self::$variant_name => $string,
                    )*
                }
            }
        }
        impl core::fmt::Display for $enum_name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(self.as_str())
            }
        }
        impl core::str::FromStr for $enum_name {
            type Err = ParseError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $(
                        $string => Ok(Self::$variant_name),
                    )*
                    _ => Err(ParseError(Self::PARSE_HELP))
                }
            }
        }
    };
}

strum! {
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Level "<off|error|warn|info|debug|trace>" {
    Off = "off",
    Error = "error",
    Warn = "warn",
    #[default]
    Info = "info",
    Debug = "debug",
    Trace = "trace",
}}

impl From<Level> for tracing_core::LevelFilter {
    fn from(value: Level) -> Self {
        match value {
            Level::Off => Self::OFF,
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

impl Formatter {
    pub const PARSE_HELP: &str = "<full|compact|pretty|json>";
}

impl FromStr for Formatter {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "full" => Self::Full,
            "compact" => Self::Compact,
            "pretty" => Self::Pretty,
            "json" => Self::Json(None),
            _ => return Err(ParseError(Self::PARSE_HELP)),
        })
    }
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

impl Timer {
    pub const PARSE_HELP: &str = "<none | local[=FORMAT] | utc[=FORMAT] | system | uptime>";
}

impl FromStr for Timer {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        alt::<_, _, winnow::error::ErrorKind, _>((
            "none".map(|_| Self::None),
            preceded("local=", rest).map(|it| Self::Local(Some(String::from(it)))),
            "local".map(|_| Self::Local(None)),
            preceded("utc=", rest).map(|it| Self::Utc(Some(String::from(it)))),
            "utc".map(|_| Self::Utc(None)),
            "system".map(|_| Self::System),
            "uptime".map(|_| Self::Uptime),
        ))
        .parse(s)
        .map_err(|_| ParseError(Self::PARSE_HELP))
    }
}

/// Write to a [`File`](std::fs::File).
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub struct File {
    pub path: PathBuf,
    pub behaviour: FileOpenBehaviour,
    /// Wrap the writer in a [`tracing_appender::non_blocking::NonBlocking`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub non_blocking: Option<NonBlocking>,
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
/// Use a [`tracing_appender::rolling::RollingFileAppender`].
pub struct Rolling {
    pub directory: PathBuf,
    pub roll: Option<Roll>,
    /// Wrap the writer in a [`tracing_appender::non_blocking::NonBlocking`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub non_blocking: Option<NonBlocking>,
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
    File(File),
    Rolling(Rolling),
}

impl Writer {
    pub const PARSE_HELP: &str = "<null | stdout | stderr | file=FILE | rolling=DIRECTORY>";
}

impl FromStr for Writer {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        alt::<_, _, winnow::error::ErrorKind, _>((
            alt(("null", "none")).map(|_| Self::Null),
            "stdout".map(|_| Self::Stdout),
            "stderr".map(|_| Self::Stderr),
            preceded("file=", rest)
                .verify(|it| !str::is_empty(it))
                .map(|it| {
                    Self::File(File {
                        path: PathBuf::from(it),
                        ..Default::default()
                    })
                }),
            preceded("rolling=", rest).map(|it| {
                Self::Rolling(Rolling {
                    directory: PathBuf::from(it),
                    ..Default::default()
                })
            }),
        ))
        .parse(s)
        .map_err(|_| ParseError(Self::PARSE_HELP))
    }
}

strum! {
/// How often to rotate the [`tracing_appender::rolling::RollingFileAppender`].
///
/// See [`tracing_appender::rolling::Rotation`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Rotation "<minutely|hourly|daily|never>" {
    Minutely = "minutely",
    Hourly = "hourly",
    Daily = "daily",
    #[default]
    Never = "never",
}}

/// Config for [`tracing_appender::rolling::RollingFileAppender`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
pub struct Roll {
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

strum! {
/// How the [`tracing_appender::non_blocking::NonBlocking`] should behave on a full queue.
///
/// See [`tracing_appender::non_blocking::NonBlockingBuilder::lossy`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum BackpressureBehaviour "<drop|block>" {
    Drop = "drop",
    Block = "block",
}}

strum! {
/// How to treat a newly created log file in [`Writer::File`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum FileOpenBehaviour "<truncate|append>" {
    #[default]
    Truncate = "truncate",
    Append = "append",
}}

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
