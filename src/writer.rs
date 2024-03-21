use std::{fs::File, io};

use tracing_appender::{
    non_blocking::{NonBlocking, NonBlockingBuilder, WorkerGuard},
    rolling::{RollingFileAppender, RollingWriter},
};

/// A thread guard in the case of [`NonBlocking`](crate::NonBlocking) config.
///
/// See [`WorkerGuard`] for more.
pub struct Guard(Option<GuardInner>);

/// Implementor of [`tracing_subscriber::fmt::MakeWriter`],
/// constructed from [`Writer`](crate::Writer) in [`Self::new`].
pub struct MakeWriter(MakeWriterInner);

/// Implementor of [`io::Write`], used by [`MakeWriter`].
pub struct Writer<'a>(WriterInner<'a>);

/// Error that can occur when constructing a writer, including e.g [`File`]-opening errors.
#[derive(Debug, thiserror::Error)]
#[error("{}: {}", .context, .source)]
pub struct Error {
    context: String,
    #[source]
    source: ErrorInner,
}

impl MakeWriter {
    pub fn new(writer: crate::Writer) -> Result<(Self, Guard), Error> {
        MakeWriterInner::new(writer).map(|(l, r)| (Self(l), Guard(r)))
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
    fn new(writer: crate::Writer) -> Result<(Self, Option<GuardInner>), Error> {
        match writer {
            crate::Writer::File {
                path,
                behaviour,
                non_blocking,
            } => {
                match match behaviour {
                    crate::FileOpenBehaviour::Truncate => File::create(&path),
                    crate::FileOpenBehaviour::Append => File::options().append(true).open(&path),
                } {
                    Ok(it) => match non_blocking {
                        Some(nb) => {
                            let (nb, g) = nb.build(it);
                            Ok((Self::NonBlocking(nb), Some(GuardInner::NonBlocking(g))))
                        }
                        None => Ok((Self::File(it), None)),
                    },
                    Err(e) => Err(Error {
                        context: format!("Couldn't open log file {}", path.display()),
                        source: ErrorInner::Io(e),
                    }),
                }
            }
            crate::Writer::Rolling {
                directory,
                rolling:
                    crate::Rolling {
                        limit,
                        prefix,
                        suffix,
                        rotation,
                    },
                non_blocking,
            } => {
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
                let builder = match rotation {
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
                            let (nb, g) = nb.build(it);
                            Ok((Self::NonBlocking(nb), Some(GuardInner::NonBlocking(g))))
                        }
                        None => Ok((Self::Rolling(it), None)),
                    },
                    Err(e) => Err(Error {
                        context: format!(
                            "Couldn't start rolling logging in directory {}",
                            directory.display()
                        ),
                        source: ErrorInner::Init(e),
                    }),
                }
            }
            crate::Writer::Stdout => Ok((Self::Stdout(io::stdout()), None)),
            crate::Writer::Stderr => Ok((Self::Stderr(io::stderr()), None)),
            crate::Writer::Null => Ok((Self::Null(io::sink()), None)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
enum ErrorInner {
    Io(io::Error),
    Init(tracing_appender::rolling::InitError),
}

enum GuardInner {
    NonBlocking(WorkerGuard),
}

enum MakeWriterInner {
    Null(io::Sink),
    NonBlocking(tracing_appender::non_blocking::NonBlocking),
    Stdout(io::Stdout),
    Stderr(io::Stderr),
    File(File),
    Rolling(RollingFileAppender),
}

enum WriterInner<'a> {
    Null(&'a io::Sink),
    NonBlocking(NonBlocking),
    Stdout(&'a io::Stdout),
    Stderr(&'a io::Stderr),
    File(&'a File),
    Rolling(RollingWriter<'a>),
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
        }
    }
}
