use tracing_core::{Event, Subscriber};
use tracing_subscriber::{
    fmt::{
        format::{Compact, Format, Full, Json, Pretty, Writer},
        FmtContext, FormatFields,
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
    N: for<'a> FormatFields<'a> + 'static,
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
        } = value;

        let orig = Format::default().with_timer(FormatTime::from(timer));
        let this = match formatter {
            crate::Formatter::Full => Self::Full(orig),
            crate::Formatter::Compact => Self::Compact(orig.compact()),
            crate::Formatter::Pretty => Self::Pretty(orig.pretty()),
            crate::Formatter::Json {
                flatten_event,
                current_span,
                span_list,
            } => Self::Json(
                orig.json()
                    .flatten_event(flatten_event)
                    .with_current_span(current_span)
                    .with_span_list(span_list),
            ),
        };

        macro_rules! map {
            ($receiver:ident.$method:ident($arg:expr)) => {
                match $receiver {
                    Self::Full(it) => Self::Full(it.$method($arg)),
                    Self::Compact(it) => Self::Compact(it.$method($arg)),
                    Self::Pretty(it) => Self::Pretty(it.$method($arg)),
                    Self::Json(it) => Self::Json(it.$method($arg)),
                }
            };
        }

        let this = map!(this.with_ansi(ansi));
        let this = map!(this.with_target(target));
        let this = map!(this.with_level(level));
        let this = map!(this.with_thread_ids(thread_ids));
        let this = map!(this.with_thread_names(thread_names));
        let this = map!(this.with_file(file));
        map!(this.with_line_number(line_number))
    }
}

enum FormatEventInner {
    Full(Format<Full, FormatTime>),
    Compact(Format<Compact, FormatTime>),
    Pretty(Format<Pretty, FormatTime>),
    Json(Format<Json, FormatTime>),
}

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for FormatEventInner
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
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
