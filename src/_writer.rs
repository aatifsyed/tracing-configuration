use std::{fs::File, io, path::PathBuf};

use tracing_appender::{
    non_blocking::{NonBlockingBuilder, WorkerGuard},
    rolling::{RollingFileAppender, RollingWriter},
};
use tracing_subscriber::fmt::writer::EitherWriter;

pub enum Writer {
    Stdout,
    Stderr,
    File {
        path: PathBuf,
        behaviour: FileOpenBehaviour,
        non_blocking: Option<NonBlocking>,
    },
    Rolling {
        directory: PathBuf,
        limit: Option<usize>,
        prefix: Option<String>,
        suffix: Option<String>,
        rotation: Rotation,
        non_blocking: Option<NonBlocking>,
    },
}

pub enum Rotation {
    Minutely,
    Hourly,
    Daily,
    Never,
}

pub enum BackpressureBehaviour {
    Drop,
    Block,
}

pub enum FileOpenBehaviour {
    Truncate,
    Append,
}

pub struct NonBlocking {
    pub buffer_length: Option<usize>,
    pub behaviour: Option<BackpressureBehaviour>,
}

pub struct Guard(GuardInner);

pub struct MakeWriter(MakeWriterInner);

impl MakeWriter {
    pub fn new(writer: Writer) -> Result<(Self, Option<Guard>), Error> {
        MakeWriterInner::new(writer).map(|(l, r)| (Self(l), r.map(Guard)))
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for MakeWriter {
    type Writer = <MakeWriterInner as tracing_subscriber::fmt::MakeWriter<'a>>::Writer;

    fn make_writer(&'a self) -> Self::Writer {
        self.0.make_writer()
    }
}

impl NonBlocking {
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
            Some(BackpressureBehaviour::Block) => builder.lossy(false),
            Some(BackpressureBehaviour::Drop) => builder.lossy(true),
            None => builder,
        };
        builder.finish(writer)
    }
}

impl MakeWriterInner {
    fn new(writer: Writer) -> Result<(Self, Option<GuardInner>), Error> {
        match writer {
            Writer::File {
                path,
                behaviour,
                non_blocking,
            } => {
                match match behaviour {
                    FileOpenBehaviour::Truncate => File::create(&path),
                    FileOpenBehaviour::Append => File::options().append(true).open(&path),
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
            Writer::Rolling {
                directory,
                limit,
                prefix,
                suffix,
                rotation,
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
                    Rotation::Minutely => {
                        builder.rotation(tracing_appender::rolling::Rotation::MINUTELY)
                    }
                    Rotation::Hourly => {
                        builder.rotation(tracing_appender::rolling::Rotation::HOURLY)
                    }
                    Rotation::Daily => builder.rotation(tracing_appender::rolling::Rotation::DAILY),
                    Rotation::Never => builder.rotation(tracing_appender::rolling::Rotation::NEVER),
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
            Writer::Stdout => Ok((Self::Stdout(io::stdout()), None)),
            Writer::Stderr => Ok((Self::Stderr(io::stderr()), None)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{}: {}", .context, .source)]
pub struct Error {
    context: String,
    #[source]
    source: ErrorInner,
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

pub enum MakeWriterInner {
    NonBlocking(tracing_appender::non_blocking::NonBlocking),
    Stdout(io::Stdout),
    Stderr(io::Stderr),
    File(File),
    Rolling(RollingFileAppender),
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for MakeWriterInner {
    type Writer = EitherWriter<
        tracing_appender::non_blocking::NonBlocking,
        EitherWriter<
            &'a io::Stdout,
            EitherWriter<&'a io::Stderr, EitherWriter<&'a File, RollingWriter<'a>>>,
        >,
    >;

    fn make_writer(&'a self) -> Self::Writer {
        match self {
            MakeWriterInner::NonBlocking(it) => EitherWriter::A(it.make_writer()),
            MakeWriterInner::Stdout(it) => EitherWriter::B(EitherWriter::A(it)),
            MakeWriterInner::Stderr(it) => EitherWriter::B(EitherWriter::B(EitherWriter::A(it))),
            MakeWriterInner::File(it) => {
                EitherWriter::B(EitherWriter::B(EitherWriter::B(EitherWriter::A(it))))
            }
            MakeWriterInner::Rolling(it) => EitherWriter::B(EitherWriter::B(EitherWriter::B(
                EitherWriter::B(it.make_writer()),
            ))),
        }
    }
}
