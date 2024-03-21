mod _filter;
mod _format;
mod _time;
mod _writer;

pub use _filter::LevelFilter;
pub use _format::Format;
pub use _time::Timer;
pub use _writer::{BackpressureBehaviour, Error, Guard, NonBlocking, Writer};
use tracing_subscriber::fmt::SubscriberBuilder;

pub fn new(filter: Vec<(String, LevelFilter)>, format: Format, writer: Writer) {
    use tracing_subscriber::{
        filter::{filter_fn, LevelFilter},
        fmt::format::FmtSpan,
        layer::SubscriberExt as _,
        util::SubscriberInitExt as _,
    };
    let _guard = tracing_subscriber::fmt()
        .with_span_events(FmtSpan::FULL)
        .with_test_writer()
        .with_max_level(LevelFilter::TRACE)
        .finish()
        .with(filter_fn(|_metadata| true))
        .set_default();
    let (writer, guard) = _writer::MakeWriter::new(writer).unwrap();
    tracing_subscriber::fmt()
        .event_format(_format::FormatEvent::from(format))
        .with_writer(writer)
        .finish()
        .with(tracing_subscriber::filter::Targets::from(
            _filter::targets::Targets(filter),
        ));
}
