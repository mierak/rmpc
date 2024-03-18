use flexi_logger::{style, FileSpec, FlexiLoggerError, LoggerHandle};

use crate::AppEvent;

pub fn init(tx: std::sync::mpsc::Sender<AppEvent>) -> Result<LoggerHandle, FlexiLoggerError> {
    #[cfg(debug_assertions)]
    return init_debug(tx);
    #[cfg(not(debug_assertions))]
    return init_release(tx);
}

#[allow(dead_code)]
fn init_release(tx: std::sync::mpsc::Sender<AppEvent>) -> Result<LoggerHandle, FlexiLoggerError> {
    flexi_logger::Logger::try_with_str("debug")?
        .log_to_file(
            FileSpec::default()
                .directory(std::env::temp_dir())
                .basename("rmpc")
                .suppress_timestamp(),
        )
        .add_writer("status_bar", Box::new(StatusBarWriter::new(tx)))
        .format_for_writer(colored_structured_detailed_format)
        .format_for_files(structured_detailed_format)
        .set_palette("1;3;15;4;13".to_string())
        .start()
}

#[allow(dead_code)]
fn init_debug(tx: std::sync::mpsc::Sender<AppEvent>) -> Result<LoggerHandle, FlexiLoggerError> {
    flexi_logger::Logger::try_with_str("debug")?
        .log_to_file_and_writer(
            FileSpec::default()
                .directory(std::env::temp_dir())
                .basename("rmpc")
                .suppress_timestamp(),
            Box::new(AppEventChannelWriter::new(tx.clone())),
        )
        .add_writer("status_bar", Box::new(StatusBarWriter::new(tx)))
        .format_for_writer(colored_structured_detailed_format)
        .format_for_files(structured_detailed_format)
        .set_palette("1;3;15;4;13".to_string())
        .start()
}

pub struct StatusBarWriter {
    tx: std::sync::mpsc::Sender<AppEvent>,
}

impl StatusBarWriter {
    pub fn new(tx: std::sync::mpsc::Sender<AppEvent>) -> Self {
        Self { tx }
    }
}

pub struct AppEventChannelWriter {
    tx: std::sync::mpsc::Sender<AppEvent>,
    format_fn: Option<flexi_logger::FormatFunction>,
}

impl flexi_logger::writers::LogWriter for StatusBarWriter {
    fn write(&self, _now: &mut flexi_logger::DeferredNow, record: &log::Record) -> std::io::Result<()> {
        match self
            .tx
            .send(AppEvent::Status(format!("{}", record.args()), record.level().into()))
        {
            Ok(v) => Ok(v),
            Err(err) => Err(std::io::Error::new(std::io::ErrorKind::Other, err)),
        }
    }

    fn flush(&self) -> std::io::Result<()> {
        Ok(())
    }
}

impl AppEventChannelWriter {
    pub fn new(tx: std::sync::mpsc::Sender<AppEvent>) -> Self {
        Self { tx, format_fn: None }
    }
}

impl flexi_logger::writers::LogWriter for AppEventChannelWriter {
    fn write(&self, now: &mut flexi_logger::DeferredNow, record: &log::Record) -> std::io::Result<()> {
        let mut buf = Vec::new();
        (self.format_fn).and_then(|fun| Some(fun(&mut buf, now, record)));

        match self.tx.send(AppEvent::Log(buf)) {
            Ok(v) => Ok(v),
            Err(err) => Err(std::io::Error::new(std::io::ErrorKind::Other, err)),
        }
    }

    fn flush(&self) -> std::io::Result<()> {
        Ok(())
    }

    fn format(&mut self, format: flexi_logger::FormatFunction) {
        self.format_fn = Some(format);
    }
}

impl From<log::Level> for crate::ui::Level {
    fn from(level: log::Level) -> Self {
        match level {
            log::Level::Error => crate::ui::Level::Error,
            log::Level::Warn => crate::ui::Level::Warn,
            log::Level::Info => crate::ui::Level::Info,
            log::Level::Debug => crate::ui::Level::Debug,
            log::Level::Trace => crate::ui::Level::Trace,
        }
    }
}

pub fn colored_structured_detailed_format(
    w: &mut dyn std::io::Write,
    now: &mut flexi_logger::DeferredNow,
    record: &log::Record,
) -> Result<(), std::io::Error> {
    let mut visitor = Visitor::new();
    record.key_values().visit(&mut visitor).unwrap();

    let level = record.level();
    write!(
        w,
        r#"{} {:<5} {}:{} message="{}" {}"#,
        now.now_utc_owned().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        record.level().to_string(),
        record.file().unwrap_or("<unnamed>"),
        record.line().unwrap_or(0),
        style(level).paint(&record.args().to_string()),
        visitor
    )
}

pub fn structured_detailed_format(
    w: &mut dyn std::io::Write,
    now: &mut flexi_logger::DeferredNow,
    record: &log::Record,
) -> Result<(), std::io::Error> {
    let mut visitor = Visitor::new();
    record.key_values().visit(&mut visitor).unwrap();
    write!(
        w,
        r#"{} {:<5} {}:{} message="{}" {}"#,
        now.now_utc_owned().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        record.level().to_string(),
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
        for ele in self.values.iter() {
            write!(f, r#"{}="{}" "#, ele.0, ele.1)?;
        }
        Ok(())
    }
}

impl<'kvs> log::kv::VisitSource<'kvs> for Visitor {
    fn visit_pair(&mut self, key: log::kv::Key<'kvs>, value: log::kv::Value<'kvs>) -> Result<(), log::kv::Error> {
        self.values.push((key.to_string(), value.to_string()));
        Ok(())
    }
}
