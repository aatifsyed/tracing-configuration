//! Configuration-as-a-struct for [`tracing_subscriber::fmt::Subscriber`], to allow
//! for serializable, dynamic configuration, at the cost of compile-time specialization.

pub mod format;
pub mod time;
pub mod writer;

#[cfg(feature = "clap4")]
use clap::{
    builder::PossibleValue,
    builder::{TypedValueParser, ValueParser, ValueParserFactory},
    ValueEnum,
};
#[cfg(feature = "schemars1")]
use schemars::JsonSchema;
#[cfg(feature = "serde1")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "serde1")]
use serde_with::*;
use std::{fmt, path::PathBuf, str::FromStr};
use tracing_subscriber::EnvFilter;
use winnow::{
    combinator::{alt, preceded},
    token::rest,
    Parser as _,
};

use writer::Guard;

/// Configuration for a totally dynamic subscriber.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
pub struct Subscriber {
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub format: Option<Format>,
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub writer: Option<Writer>,
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub filter: Option<Filter>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
pub struct Filter {
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub regex: Option<bool>,
    #[cfg_attr(
        feature = "serde1",
        serde(
            skip_serializing_if = "Vec::is_empty",
            with = "As::<Vec<DisplayFromStr>>"
        )
    )]
    #[cfg_attr(feature = "schemars1", schemars(with = "Vec<String>"))]
    pub directives: Vec<tracing_subscriber::filter::Directive>,
}

impl From<Filter> for EnvFilter {
    fn from(value: Filter) -> Self {
        let Filter { regex, directives } = value;
        let mut builder = EnvFilter::builder();
        if let Some(regex) = regex {
            builder = builder.with_regex(regex)
        }
        directives
            .into_iter()
            .fold(builder.parse_lossy(""), EnvFilter::add_directive)
    }
}

#[derive(Debug)]
pub struct ParseError(&'static str);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for ParseError {}

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
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
pub struct Format {
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_ansi`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub ansi: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_target`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub target: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_level`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub level: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_thread_ids`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub thread_ids: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_thread_names`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub thread_names: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_file`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub file: Option<bool>,
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::with_line_number`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub line_number: Option<bool>,
    /// Specific output formats.
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub formatter: Option<Formatter>,
    /// What timing information to include.
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub timer: Option<Timer>,
}

/// The specific output format.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[cfg_attr(feature = "serde1", serde(rename_all = "lowercase"))]
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

impl FromStr for Formatter {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "full" => Self::Full,
            "compact" => Self::Compact,
            "pretty" => Self::Pretty,
            "json" => Self::Json(None),
            _ => {
                return Err(ParseError(
                    "Expected one of `full`, `compact`, `pretty`, or `json`",
                ))
            }
        })
    }
}

#[cfg(feature = "clap4")]
impl ValueEnum for Formatter {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Full, Self::Compact, Self::Pretty, Self::Json(None)]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Formatter::Full => PossibleValue::new("full"),
            Formatter::Compact => PossibleValue::new("compact"),
            Formatter::Pretty => PossibleValue::new("pretty"),
            Formatter::Json(_) => PossibleValue::new("json"),
        })
    }
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
pub struct Json {
    /// See [`tracing_subscriber::fmt::format::Json::flatten_event`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub flatten_event: Option<bool>,
    /// See [`tracing_subscriber::fmt::format::Json::with_current_span`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub current_span: Option<bool>,
    /// See [`tracing_subscriber::fmt::format::Json::with_span_list`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub span_list: Option<bool>,
}

/// Which timer implementation to use.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[cfg_attr(feature = "serde1", serde(rename_all = "lowercase"))]
pub enum Timer {
    /// See [`tracing_subscriber::fmt::SubscriberBuilder::without_time`].
    None,
    /// See [`tracing_subscriber::fmt::time::ChronoLocal`].
    Local(
        #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
        Option<String>,
    ),
    /// See [`tracing_subscriber::fmt::time::ChronoUtc`].
    Utc(
        #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
        Option<String>,
    ),
    /// See [`tracing_subscriber::fmt::time::SystemTime`].
    #[default]
    System,
    /// See [`tracing_subscriber::fmt::time::Uptime`].
    Uptime,
}

impl Timer {
    const PARSE_ERROR: &str = "Expected one of `none`, `local`, `local=<format>`, `utc`, `utc=<format>`, `system`, or `uptime`";
}

impl FromStr for Timer {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        alt::<_, _, winnow::error::EmptyError, _>((
            "none".map(|_| Self::None),
            preceded("local=", rest).map(|it| Self::Local(Some(String::from(it)))),
            "local".map(|_| Self::Local(None)),
            preceded("utc=", rest).map(|it| Self::Utc(Some(String::from(it)))),
            "utc".map(|_| Self::Utc(None)),
            "system".map(|_| Self::System),
            "uptime".map(|_| Self::Uptime),
        ))
        .parse(s)
        .map_err(|_| ParseError(Self::PARSE_ERROR))
    }
}

#[cfg(feature = "clap4")]
impl ValueEnum for Timer {
    fn value_variants<'a>() -> &'a [Self] {
        const {
            &[
                Timer::None,
                Timer::Local(None),
                Timer::Local(Some(String::new())),
                Timer::Utc(None),
                Timer::Utc(Some(String::new())),
                Timer::System,
                Timer::Uptime,
            ]
        }
    }
    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Timer::None => PossibleValue::new("none"),
            Timer::Local(None) => PossibleValue::new("local"),
            Timer::Local(Some(_)) => PossibleValue::new("local=<format>"),
            Timer::Utc(None) => PossibleValue::new("utc"),
            Timer::Utc(Some(_)) => PossibleValue::new("utc=<format>"),
            Timer::System => PossibleValue::new("system"),
            Timer::Uptime => PossibleValue::new("uptime"),
        })
    }
    fn from_str(input: &str, _ignore_case: bool) -> Result<Self, String> {
        input.parse().map_err(|ParseError(it)| String::from(it))
    }
}

/// Write to a [`File`](std::fs::File).
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[cfg_attr(feature = "serde1", serde(rename_all = "lowercase"))]
pub struct File {
    pub path: PathBuf,
    pub mode: FileOpenMode,
    /// Wrap the writer in a [`tracing_appender::non_blocking::NonBlocking`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub non_blocking: Option<NonBlocking>,
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[cfg_attr(feature = "serde1", serde(rename_all = "lowercase"))]
/// Use a [`tracing_appender::rolling::RollingFileAppender`].
pub struct Rolling {
    pub directory: PathBuf,
    pub roll: Option<Roll>,
    /// Wrap the writer in a [`tracing_appender::non_blocking::NonBlocking`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub non_blocking: Option<NonBlocking>,
}

/// Which writer to use.
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[cfg_attr(feature = "serde1", serde(rename_all = "lowercase"))]
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
    const PARSE_ERROR: &str =
        "Expected one of `null`, `stdout`, `stderr`, `file=<file>`, or `rolling=<directory>`";
}

impl FromStr for Writer {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        alt::<_, _, winnow::error::EmptyError, _>((
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
        .map_err(|_| ParseError(Self::PARSE_ERROR))
    }
}

#[cfg(feature = "clap4")]
// can't `const { PathBuf::new() }` so this is what we need
impl ValueParserFactory for Writer {
    type Parser = ValueParser;
    fn value_parser() -> Self::Parser {
        #[derive(Clone)]
        struct _TypedValueParser;
        impl TypedValueParser for _TypedValueParser {
            type Value = Writer;
            fn parse_ref(
                &self,
                cmd: &clap::Command,
                _arg: Option<&clap::Arg>,
                value: &std::ffi::OsStr,
            ) -> Result<Self::Value, clap::Error> {
                value
                    .to_str()
                    .ok_or(clap::Error::new(clap::error::ErrorKind::InvalidUtf8))?
                    .parse()
                    .map_err(|_| {
                        clap::Error::new(clap::error::ErrorKind::InvalidValue).with_cmd(cmd)
                    })
            }
            fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue> + '_>> {
                Some(Box::new(
                    [
                        PossibleValue::new("null"),
                        PossibleValue::new("stdout"),
                        PossibleValue::new("stderr"),
                        PossibleValue::new("file=<file>"),
                        PossibleValue::new("rolling=<directory>"),
                    ]
                    .into_iter(),
                ))
            }
        }
        ValueParser::new(_TypedValueParser)
    }
}

strum_lite::strum! {
/// How often to rotate the [`tracing_appender::rolling::RollingFileAppender`].
///
/// See [`tracing_appender::rolling::Rotation`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[cfg_attr(feature = "serde1", serde(rename_all = "lowercase"))]
#[cfg_attr(feature = "clap4", derive(ValueEnum))]
pub enum Rotation {
    Minutely = "minutely",
    Hourly = "hourly",
    Daily = "daily",
    #[default]
    Never = "never",
}}

/// Config for [`tracing_appender::rolling::RollingFileAppender`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
pub struct Roll {
    /// See [`tracing_appender::rolling::Builder::max_log_files`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub limit: Option<usize>,
    /// See [`tracing_appender::rolling::Builder::filename_prefix`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub prefix: Option<String>,
    /// See [`tracing_appender::rolling::Builder::filename_suffix`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub suffix: Option<String>,
    /// See [`tracing_appender::rolling::Builder::rotation`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub rotation: Option<Rotation>,
}

strum_lite::strum! {
/// How the [`tracing_appender::non_blocking::NonBlocking`] should behave on a full queue.
///
/// See [`tracing_appender::non_blocking::NonBlockingBuilder::lossy`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[cfg_attr(feature = "serde1", serde(rename_all = "lowercase"))]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
pub enum BackpressureBehaviour {
    Drop = "drop",
    Block = "block",
}}

strum_lite::strum! {
/// How to treat a newly created log file in [`Writer::File`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[cfg_attr(feature = "serde1", serde(rename_all = "lowercase"))]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
pub enum FileOpenMode {
    #[default]
    Truncate = "truncate",
    Append = "append",
}}

/// Configuration for [`tracing_appender::non_blocking::NonBlocking`].
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[cfg_attr(feature = "serde1", serde(rename_all = "lowercase"))]
pub struct NonBlocking {
    /// See [`tracing_appender::non_blocking::NonBlockingBuilder::buffered_lines_limit`].
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub buffer_length: Option<usize>,
    #[cfg_attr(feature = "serde1", serde(skip_serializing_if = "Option::is_none"))]
    pub behaviour: Option<BackpressureBehaviour>,
}

#[cfg(all(test, feature = "schemars1"))]
#[test]
fn schema() {
    let s = serde_json::to_string_pretty(&schemars::schema_for!(Subscriber)).unwrap();
    expect_test::expect_file!["../snapshots/schema.json"].assert_eq(&s);
}
