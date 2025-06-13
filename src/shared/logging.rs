use std::time::Duration;

use crossbeam::channel::Sender;
use flexi_logger::{FileSpec, FlexiLoggerError, LoggerHandle};

use super::events::Level;
use crate::AppEvent;

pub fn init(tx: Sender<AppEvent>) -> Result<LoggerHandle, FlexiLoggerError> {
    #[cfg(debug_assertions)]
    return init_debug(tx);
    #[cfg(not(debug_assertions))]
    return init_release(tx);
}

pub fn init_console() -> Result<LoggerHandle, FlexiLoggerError> {
    flexi_logger::Logger::try_with_env_or_str("warn")?
        .log_to_stderr()
        // status bar is replicated to the normal log file so it is safe to drop
        .add_writer("status_bar", Box::new(NullWriter))
        .format_for_stderr(console_format)
        .set_palette("1;3;15;4;13".to_string())
        .start()
}

#[allow(dead_code)]
fn init_release(tx: Sender<AppEvent>) -> Result<LoggerHandle, FlexiLoggerError> {
    let uid = rustix::process::geteuid();
    flexi_logger::Logger::try_with_env_or_str("debug")?
        .log_to_file(
            FileSpec::default()
                .directory(std::env::temp_dir())
                .basename(format!("rmpc_{}", uid.as_raw()))
                .suppress_timestamp(),
        )
        .add_writer("status_bar", Box::new(StatusBarWriter::new(tx)))
        .format_for_files(structured_detailed_format)
        .set_palette("1;3;15;4;13".to_string())
        .start()
}

#[allow(dead_code)]
fn init_debug(tx: Sender<AppEvent>) -> Result<LoggerHandle, FlexiLoggerError> {
    let uid = rustix::process::geteuid();
    flexi_logger::Logger::try_with_env_or_str("debug")?
        .log_to_file_and_writer(
            FileSpec::default()
                .directory(std::env::temp_dir())
                .basename(format!("rmpc_{}", uid.as_raw()))
                .suppress_timestamp(),
            Box::new(AppEventChannelWriter::new(tx.clone())),
        )
        .add_writer("status_bar", Box::new(StatusBarWriter::new(tx)))
        .format_for_writer(structured_detailed_format)
        .format_for_files(structured_detailed_format)
        .set_palette("1;3;15;4;13".to_string())
        .start()
}

pub struct NullWriter;
impl flexi_logger::writers::LogWriter for NullWriter {
    fn write(
        &self,
        _now: &mut flexi_logger::DeferredNow,
        _record: &log::Record,
    ) -> std::io::Result<()> {
        Ok(())
    }

    fn flush(&self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct StatusBarWriter {
    tx: Sender<AppEvent>,
}

impl StatusBarWriter {
    pub fn new(tx: Sender<AppEvent>) -> Self {
        Self { tx }
    }
}

pub struct AppEventChannelWriter {
    tx: Sender<AppEvent>,
    format_fn: Option<flexi_logger::FormatFunction>,
}

impl flexi_logger::writers::LogWriter for StatusBarWriter {
    fn write(
        &self,
        _now: &mut flexi_logger::DeferredNow,
        record: &log::Record,
    ) -> std::io::Result<()> {
        match self.tx.send(AppEvent::Status(
            format!("{}", record.args()),
            record.level().into(),
            Duration::from_secs(5),
        )) {
            Ok(v) => Ok(v),
            Err(err) => Err(std::io::Error::other(err)),
        }
    }

    fn flush(&self) -> std::io::Result<()> {
        Ok(())
    }
}

impl AppEventChannelWriter {
    pub fn new(tx: Sender<AppEvent>) -> Self {
        Self { tx, format_fn: None }
    }
}

impl flexi_logger::writers::LogWriter for AppEventChannelWriter {
    fn write(
        &self,
        now: &mut flexi_logger::DeferredNow,
        record: &log::Record,
    ) -> std::io::Result<()> {
        let mut buf = Vec::new();
        (self.format_fn).map(|fun| fun(&mut buf, now, record));

        match self.tx.send(AppEvent::Log(buf)) {
            Ok(v) => Ok(v),
            Err(err) => Err(std::io::Error::other(err)),
        }
    }

    fn flush(&self) -> std::io::Result<()> {
        Ok(())
    }

    fn format(&mut self, format: flexi_logger::FormatFunction) {
        self.format_fn = Some(format);
    }
}

impl From<log::Level> for Level {
    fn from(level: log::Level) -> Self {
        match level {
            log::Level::Error => Level::Error,
            log::Level::Warn => Level::Warn,
            log::Level::Info => Level::Info,
            log::Level::Debug => Level::Debug,
            log::Level::Trace => Level::Trace,
        }
    }
}

pub fn console_format(
    w: &mut dyn std::io::Write,
    _now: &mut flexi_logger::DeferredNow,
    record: &log::Record,
) -> anyhow::Result<(), std::io::Error> {
    let mut visitor = Visitor::new();
    match record.key_values().visit(&mut visitor) {
        Ok(()) => {}
        Err(err) => {
            return Err(std::io::Error::other(err));
        }
    }
    let level = record.level();

    write!(
        w,
        r"{:<5}: {} {}",
        flexi_logger::style(level).paint(level.to_string()),
        &record.args().to_string(),
        visitor
    )
}

pub fn structured_detailed_format(
    w: &mut dyn std::io::Write,
    now: &mut flexi_logger::DeferredNow,
    record: &log::Record,
) -> anyhow::Result<(), std::io::Error> {
    let mut visitor = Visitor::new();
    match record.key_values().visit(&mut visitor) {
        Ok(()) => {}
        Err(err) => {
            return Err(std::io::Error::other(err));
        }
    }
    write!(
        w,
        r#"{} {:<5} thread={} {}:{} message="{}" {}"#,
        now.now_utc_owned().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        record.level().to_string(),
        std::thread::current().name().unwrap_or("<unnamed>"),
        record.file().unwrap_or("<unnamed>"),
        record.line().unwrap_or(0),
        &record.args().to_string(),
        visitor
    )
}

#[derive(Debug)]
struct Visitor {
    values: Vec<(String, String)>,
}

impl Visitor {
    fn new() -> Self {
        Self { values: Vec::new() }
    }
}

impl std::fmt::Display for Visitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for ele in &self.values {
            write!(f, r#"{}="{}" "#, ele.0, ele.1)?;
        }
        Ok(())
    }
}

impl<'kvs> log::kv::VisitSource<'kvs> for Visitor {
    fn visit_pair(
        &mut self,
        key: log::kv::Key<'kvs>,
        value: log::kv::Value<'kvs>,
    ) -> Result<(), log::kv::Error> {
        self.values.push((key.to_string(), value.to_string()));
        Ok(())
    }
}
