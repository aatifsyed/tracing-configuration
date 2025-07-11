use tracing_core::{Event, Subscriber};
use tracing_subscriber::{
    fmt::{
        format::{
            Compact, DefaultFields, Format, Full, Json, JsonFields, Pretty, PrettyFields, Writer,
        },
        FmtContext,
    },
    registry::LookupSpan,
};

use crate::time::FormatTime;

/// Implementor of [`tracing_subscriber::fmt::FormatEvent`], constructed [`From`] [`Format`](crate::Format).
pub struct FormatEvent(FormatEventInner);

impl From<crate::Format> for FormatEvent {
    fn from(value: crate::Format) -> Self {
        Self(value.into())
    }
}

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for FormatEvent
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        self.0.format_event(ctx, writer, event)
    }
}

/// Implementor of [`tracing_subscriber::fmt::FormatFields`], constructed [`From`] [`Formatter`](crate::Formatter).
pub struct FormatFields(FormatFieldsInner);

impl From<crate::Formatter> for FormatFields {
    fn from(value: crate::Formatter) -> Self {
        Self(value.into())
    }
}

impl<'writer> tracing_subscriber::fmt::FormatFields<'writer> for FormatFields {
    fn format_fields<R: tracing_subscriber::field::RecordFields>(
        &self,
        writer: Writer<'writer>,
        fields: R,
    ) -> std::fmt::Result {
        self.0.format_fields(writer, fields)
    }
}

enum FormatEventInner {
    Full(Format<Full, FormatTime>),
    Compact(Format<Compact, FormatTime>),
    Pretty(Format<Pretty, FormatTime>),
    Json(Format<Json, FormatTime>),
}

impl From<crate::Format> for FormatEventInner {
    fn from(value: crate::Format) -> Self {
        let crate::Format {
            ansi,
            target,
            level,
            thread_ids,
            thread_names,
            file,
            line_number,
            formatter,
            timer,
            span_events: _, // handled out-of-band
        } = value;

        let orig = Format::default().with_timer(FormatTime::from(timer.unwrap_or_default()));
        let mut this = match formatter.unwrap_or_default() {
            crate::Formatter::Full => Self::Full(orig),
            crate::Formatter::Compact => Self::Compact(orig.compact()),
            crate::Formatter::Pretty => Self::Pretty(orig.pretty()),
            crate::Formatter::Json(it) => Self::Json({
                let crate::Json {
                    flatten_event,
                    current_span,
                    span_list,
                } = it.unwrap_or_default();
                let mut this = orig.json();
                if let Some(it) = flatten_event {
                    this = this.flatten_event(it)
                }
                if let Some(it) = current_span {
                    this = this.with_current_span(it)
                }
                if let Some(it) = span_list {
                    this = this.with_span_list(it)
                }
                this
            }),
        };

        macro_rules! apply {
            ($receiver:ident.$method:ident($arg:expr)) => {
                if let Some(arg) = $arg {
                    $receiver = match $receiver {
                        Self::Full(it) => Self::Full(it.$method(arg)),
                        Self::Compact(it) => Self::Compact(it.$method(arg)),
                        Self::Pretty(it) => Self::Pretty(it.$method(arg)),
                        Self::Json(it) => Self::Json(it.$method(arg)),
                    };
                }
            };
        }

        apply!(this.with_ansi(ansi));
        apply!(this.with_target(target));
        apply!(this.with_level(level));
        apply!(this.with_thread_ids(thread_ids));
        apply!(this.with_thread_names(thread_names));
        apply!(this.with_file(file));
        apply!(this.with_line_number(line_number));

        this
    }
}

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for FormatEventInner
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::format::FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        match self {
            FormatEventInner::Full(it) => it.format_event(ctx, writer, event),
            FormatEventInner::Compact(it) => it.format_event(ctx, writer, event),
            FormatEventInner::Pretty(it) => it.format_event(ctx, writer, event),
            FormatEventInner::Json(it) => it.format_event(ctx, writer, event),
        }
    }
}

enum FormatFieldsInner {
    Default(DefaultFields),
    Json(JsonFields),
    Pretty(PrettyFields),
}

impl From<crate::Formatter> for FormatFieldsInner {
    fn from(value: crate::Formatter) -> Self {
        match value {
            crate::Formatter::Full => Self::Default(DefaultFields::new()),
            crate::Formatter::Compact => Self::Default(DefaultFields::new()),
            crate::Formatter::Pretty => Self::Pretty(PrettyFields::new()),
            crate::Formatter::Json { .. } => Self::Json(JsonFields::new()),
        }
    }
}

impl<'writer> tracing_subscriber::fmt::FormatFields<'writer> for FormatFieldsInner {
    fn format_fields<R: tracing_subscriber::field::RecordFields>(
        &self,
        writer: Writer<'writer>,
        fields: R,
    ) -> std::fmt::Result {
        match self {
            FormatFieldsInner::Default(it) => it.format_fields(writer, fields),
            FormatFieldsInner::Json(it) => it.format_fields(writer, fields),
            FormatFieldsInner::Pretty(it) => it.format_fields(writer, fields),
        }
    }
}
