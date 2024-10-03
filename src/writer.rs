use std::{error::Error as _, fmt, fs::File, io, sync::Arc};

use tracing_appender::{
    non_blocking::{NonBlocking, NonBlockingBuilder, WorkerGuard},
    rolling::{RollingFileAppender, RollingWriter},
};

/// A thread guard in the case of [`NonBlocking`](crate::NonBlocking) config.
///
/// See [`WorkerGuard`] for more.
pub struct Guard {
    _guard: Option<GuardInner>,
}

/// Implementor of [`tracing_subscriber::fmt::MakeWriter`],
/// constructed from [`Writer`](crate::Writer) in [`Self::new`].
pub struct MakeWriter(MakeWriterInner);

/// Implementor of [`io::Write`], used by [`MakeWriter`].
pub struct Writer<'a>(WriterInner<'a>);

/// Error that can occur when constructing a writer, including e.g [`File`]-opening errors.
#[derive(Debug)]
pub struct Error(io::Error);

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl MakeWriter {
    /// Create a new [`MakeWriter`], and a [`Guard`] that handles e.g flushing [`NonBlocking`] IO.
    ///
    /// Errors when opening files or directories are deferred for the subscriber to handle (typically by logging).
    /// If you wish to handle them yourself, see [`Self::try_new`].
    pub fn new(writer: crate::Writer) -> (Self, Guard) {
        let (this, _guard) = MakeWriterInner::new(writer, true).expect("errors have been deferred");
        (Self(this), Guard { _guard })
    }
    /// Create a new [`MakeWriter`].
    ///
    /// Returns [`Err`] if e.g opening a log file fails.
    /// If you wish the subscriber to handle them (typically by logging), see [`Self::new`].
    pub fn try_new(writer: crate::Writer) -> Result<(Self, Guard), Error> {
        MakeWriterInner::new(writer, false).map(|(l, r)| (Self(l), Guard { _guard: r }))
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for MakeWriter {
    type Writer = Writer<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        Writer(self.0.make_writer())
    }
}

impl io::Write for Writer<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl crate::NonBlocking {
    fn build<T: io::Write + Send + 'static>(
        &self,
        writer: T,
    ) -> (tracing_appender::non_blocking::NonBlocking, WorkerGuard) {
        let Self {
            buffer_length,
            behaviour,
        } = self;
        let mut builder = NonBlockingBuilder::default();
        if let Some(it) = buffer_length {
            builder = builder.buffered_lines_limit(*it)
        }
        let builder = match behaviour {
            Some(crate::BackpressureBehaviour::Block) => builder.lossy(false),
            Some(crate::BackpressureBehaviour::Drop) => builder.lossy(true),
            None => builder,
        };
        builder.finish(writer)
    }
}

impl MakeWriterInner {
    fn new(writer: crate::Writer, defer: bool) -> Result<(Self, Option<GuardInner>), Error> {
        match writer {
            crate::Writer::File(crate::File {
                path,
                behaviour,
                non_blocking,
            }) => {
                match match behaviour {
                    crate::FileOpenBehaviour::Truncate => File::create(&path),
                    crate::FileOpenBehaviour::Append => File::options().append(true).open(&path),
                } {
                    Ok(it) => match non_blocking {
                        Some(nb) => {
                            let (nb, _guard) = nb.build(it);
                            Ok((
                                Self::NonBlocking(nb),
                                Some(GuardInner::NonBlocking { _guard }),
                            ))
                        }
                        None => Ok((Self::File(it), None)),
                    },
                    Err(e) => {
                        let e = io_extra::context(
                            e,
                            format!("couldn't open log file {}", path.display()),
                        );
                        match defer {
                            true => Ok((Self::Deferred(Arc::new(e)), None)),
                            false => Err(Error(e)),
                        }
                    }
                }
            }
            crate::Writer::Rolling(crate::Rolling {
                directory,
                roll: rolling,
                non_blocking,
            }) => {
                let crate::Roll {
                    limit,
                    prefix,
                    suffix,
                    rotation,
                } = rolling.unwrap_or_default();
                let mut builder = RollingFileAppender::builder();
                if let Some(limit) = limit {
                    builder = builder.max_log_files(limit)
                }
                if let Some(prefix) = prefix {
                    builder = builder.filename_prefix(prefix)
                }
                if let Some(suffix) = suffix {
                    builder = builder.filename_suffix(suffix)
                }
                let builder = match rotation.unwrap_or_default() {
                    crate::Rotation::Minutely => {
                        builder.rotation(tracing_appender::rolling::Rotation::MINUTELY)
                    }
                    crate::Rotation::Hourly => {
                        builder.rotation(tracing_appender::rolling::Rotation::HOURLY)
                    }
                    crate::Rotation::Daily => {
                        builder.rotation(tracing_appender::rolling::Rotation::DAILY)
                    }
                    crate::Rotation::Never => {
                        builder.rotation(tracing_appender::rolling::Rotation::NEVER)
                    }
                };

                match builder.build(&directory) {
                    Ok(it) => match non_blocking {
                        Some(nb) => {
                            let (nb, _guard) = nb.build(it);
                            Ok((
                                Self::NonBlocking(nb),
                                Some(GuardInner::NonBlocking { _guard }),
                            ))
                        }
                        None => Ok((Self::Rolling(it), None)),
                    },
                    Err(e) => {
                        let kind = e
                            .source()
                            .and_then(|it| it.downcast_ref::<io::Error>())
                            .map(io::Error::kind)
                            .unwrap_or(io::ErrorKind::Other);
                        let e = io_extra::context(
                            io::Error::new(kind, e),
                            format!(
                                "couldn't start logging in directory {}",
                                directory.display()
                            ),
                        );
                        match defer {
                            true => Ok((Self::Deferred(Arc::new(e)), None)),
                            false => Err(Error(e)),
                        }
                    }
                }
            }
            crate::Writer::Stdout => Ok((Self::Stdout(io::stdout()), None)),
            crate::Writer::Stderr => Ok((Self::Stderr(io::stderr()), None)),
            crate::Writer::Null => Ok((Self::Null(io::sink()), None)),
        }
    }
}

enum GuardInner {
    NonBlocking { _guard: WorkerGuard },
}

enum MakeWriterInner {
    Null(io::Sink),
    NonBlocking(tracing_appender::non_blocking::NonBlocking),
    Stdout(io::Stdout),
    Stderr(io::Stderr),
    File(File),
    Rolling(RollingFileAppender),
    Deferred(Arc<io::Error>),
}

enum WriterInner<'a> {
    Null(&'a io::Sink),
    NonBlocking(NonBlocking),
    Stdout(&'a io::Stdout),
    Stderr(&'a io::Stderr),
    File(&'a File),
    Rolling(RollingWriter<'a>),
    Deferred(&'a Arc<io::Error>),
}

impl io::Write for WriterInner<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            WriterInner::NonBlocking(it) => it.write(buf),
            WriterInner::Stdout(it) => it.write(buf),
            WriterInner::Stderr(it) => it.write(buf),
            WriterInner::File(it) => it.write(buf),
            WriterInner::Rolling(it) => it.write(buf),
            WriterInner::Null(it) => it.write(buf),
            WriterInner::Deferred(e) => Err(io::Error::new(e.kind(), Arc::clone(e))),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            WriterInner::NonBlocking(it) => it.flush(),
            WriterInner::Stdout(it) => it.flush(),
            WriterInner::Stderr(it) => it.flush(),
            WriterInner::File(it) => it.flush(),
            WriterInner::Rolling(it) => it.flush(),
            WriterInner::Null(it) => it.flush(),
            WriterInner::Deferred(e) => Err(io::Error::new(e.kind(), Arc::clone(e))),
        }
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for MakeWriterInner {
    type Writer = WriterInner<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        match self {
            MakeWriterInner::NonBlocking(it) => Self::Writer::NonBlocking(it.make_writer()),
            MakeWriterInner::Stdout(it) => Self::Writer::Stdout(it),
            MakeWriterInner::Stderr(it) => Self::Writer::Stderr(it),
            MakeWriterInner::File(it) => Self::Writer::File(it.make_writer()),
            MakeWriterInner::Rolling(it) => Self::Writer::Rolling(it.make_writer()),
            MakeWriterInner::Null(it) => Self::Writer::Null(it),
            MakeWriterInner::Deferred(it) => Self::Writer::Deferred(it),
        }
    }
}
