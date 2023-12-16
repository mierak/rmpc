use std::{
    io::{self, Write},
    vec,
};

use time::{macros::format_description, UtcOffset};
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
        println!("{buf:?}");
        Ok(buf_len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn configure(level: Level, tx: &std::sync::mpsc::Sender<AppEvent>) -> Vec<WorkerGuard> {
    let error_writer = Box::leak(Box::new(LogChannelWriter::new(tx.clone(), WriterVariant::StatusBar)));
    let file_appender = tracing_appender::rolling::RollingFileAppender::new(Rotation::DAILY, "./", "mpdox.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let (non_blocking_errors, errors_guard) = tracing_appender::non_blocking(&*error_writer);
    #[cfg(debug_assertions)]
    let mut guards = vec![guard, errors_guard];
    #[cfg(not(debug_assertions))]
    let guards = vec![guard, errors_guard];
    let registry = tracing_subscriber::registry()
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
                .with_filter(LogsFilter::new(level)),
        )
        .with(
            tracing_subscriber::fmt::Layer::default()
                .with_writer(non_blocking_errors)
                .with_target(false)
                .with_level(false)
                .without_time()
                .with_filter(LogsFilter::new(Level::ERROR)),
        );
    #[cfg(debug_assertions)]
    {
        let logs_writer = Box::leak(Box::new(LogChannelWriter::new(tx.clone(), WriterVariant::Log)));
        let (non_blocking_logs, logs_guard) = tracing_appender::non_blocking(&*logs_writer);
        guards.push(logs_guard);
        registry
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
            .init();
    }
    #[cfg(not(debug_assertions))]
    {
        registry.init();
    }

    guards
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

    fn enabled(&self, meta: &Metadata<'_>, _cx: &Context<'_, T>) -> bool {
        meta.target().contains(clap::crate_name!()) && *meta.level() <= self.level
    }
}

pub enum WriterVariant {
    Log,
    StatusBar,
}

pub struct LogChannelWriter {
    tx: std::sync::mpsc::Sender<AppEvent>,
    variant: WriterVariant,
}

impl LogChannelWriter {
    pub fn new(tx: std::sync::mpsc::Sender<AppEvent>, variant: WriterVariant) -> Self {
        Self { tx, variant }
    }
}

impl Write for &LogChannelWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if (match self.variant {
            WriterVariant::Log => self.tx.send(AppEvent::Log(buf.to_owned())),
            WriterVariant::StatusBar => self
                .tx
                .send(AppEvent::StatusBar(String::from_utf8_lossy(buf).to_string())),
        })
        .is_err()
        {
            return Err(io::Error::new(io::ErrorKind::Other, anyhow::anyhow!("test")));
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
