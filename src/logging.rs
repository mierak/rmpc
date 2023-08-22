use std::{
    io::{self, Write},
    vec,
};

use time::{macros::format_description, UtcOffset};
use tokio::sync::mpsc::Sender;
use tracing::{subscriber::Interest, Level, Metadata};
use tracing_appender::{non_blocking::WorkerGuard, rolling::Rotation};
use tracing_subscriber::{
    fmt::MakeWriter,
    layer::{Context, Filter},
    prelude::__tracing_subscriber_SubscriberExt,
    util::SubscriberInitExt,
    Layer,
};

use crate::AppEvent;

struct TestWriter;

impl std::io::Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let buf_len = buf.len();
        println!("{:?}", buf);
        Ok(buf_len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn configure(level: Level, tx: Sender<AppEvent>) -> Vec<WorkerGuard> {
    let error_writer = Box::leak(Box::new(LogChannelWriter::new(tx.clone(), WriterVariant::StatusBar)));
    let logs_writer = Box::leak(Box::new(LogChannelWriter::new(tx.clone(), WriterVariant::Log)));
    let file_appender = tracing_appender::rolling::RollingFileAppender::new(Rotation::DAILY, "./", "mpdox.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let (non_blocking_errors, _errors_guard) = tracing_appender::non_blocking(&*error_writer);
    let (non_blocking_logs, _logs_guard) = tracing_appender::non_blocking(&*logs_writer);
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::Layer::default()
                .with_writer(non_blocking_errors)
                .with_target(false)
                .with_level(false)
                .without_time()
                .with_filter(LogsFilter::new(Level::ERROR)),
        )
        .with(
            tracing_subscriber::fmt::Layer::default()
                .with_writer(non_blocking_logs)
                .with_ansi(true)
                .with_file(true)
                .with_target(false)
                .with_line_number(true)
                .with_timer(tracing_subscriber::fmt::time::OffsetTime::new(
                    UtcOffset::UTC,
                    format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"),
                ))
                .compact()
                .with_filter(LogsFilter::new(level)),
        )
        .with(
            tracing_subscriber::fmt::Layer::default()
                .with_ansi(false)
                .with_writer(non_blocking)
                .with_file(true)
                .with_target(false)
                .with_line_number(true)
                .with_timer(tracing_subscriber::fmt::time::OffsetTime::new(
                    UtcOffset::UTC,
                    format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"),
                ))
                .compact()
                .with_filter(LogsFilter::new(Level::TRACE)),
        )
        .init();

    vec![_guard, _errors_guard, _logs_guard]
}

pub struct LogsFilter {
    level: Level,
}
impl LogsFilter {
    pub fn new(level: Level) -> Self {
        LogsFilter { level }
    }
}
impl<T> Filter<T> for LogsFilter {
    fn callsite_enabled(&self, meta: &'static Metadata<'static>) -> Interest {
        if meta.target().contains(clap::crate_name!()) {
            Interest::sometimes()
        } else {
            Interest::never()
        }
    }

    fn enabled(&self, _meta: &Metadata<'_>, _cx: &Context<'_, T>) -> bool {
        _meta.target().contains(clap::crate_name!()) && *_meta.level() <= self.level
    }
}

pub enum WriterVariant {
    Log,
    StatusBar,
}

pub struct LogChannelWriter {
    tx: tokio::sync::mpsc::Sender<AppEvent>,
    variant: WriterVariant,
}

impl LogChannelWriter {
    pub fn new(tx: tokio::sync::mpsc::Sender<AppEvent>, variant: WriterVariant) -> Self {
        Self { tx, variant }
    }
}

impl Write for &LogChannelWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.variant {
            WriterVariant::Log => {
                self.tx.try_send(AppEvent::Log(buf.to_owned())).unwrap();
            }
            WriterVariant::StatusBar => {
                self.tx
                    .try_send(AppEvent::StatusBar(String::from_utf8_lossy(buf).to_string()))
                    .unwrap();
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for LogChannelWriter {
    type Writer = &'a Self;

    fn make_writer(&'a self) -> Self::Writer {
        self
    }

    fn make_writer_for(&'a self, _metadata: &Metadata<'_>) -> Self::Writer {
        self
    }
}

// -------
// struct LogBufMakeWriter<'a> {
//     buf: &'a Vec<String>,
// }
//
// impl<'a> LogBufMakeWriter<'a> {
//     fn new(buf: &'a Vec<String>) -> Self {
//         Self { buf }
//     }
// }
//
// impl<'a> MakeWriter<'a> for LogBufMakeWriter<'a> {
//     type Writer = LogBufWriter<'a>;
//
//     fn make_writer(&'a self) -> Self::Writer {
//         LogBufWriter { buf: self.buf }
//     }
//
//     fn make_writer_for(&'a self, _metadata: &Metadata<'_>) -> Self::Writer {
//         LogBufWriter { buf: self.buf }
//     }
// }
//
// struct LogBufWriter<'a> {
//     buf: &'a mut Vec<String>,
// }
//
// impl<'a> Write for LogBufWriter<'a> {
//     fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
//         self.buf.push(String::from_utf8_lossy(buf).to_string());
//         Ok(buf.len())
//     }
//
//     fn flush(&mut self) -> io::Result<()> {
//         Ok(())
//     }
// }
