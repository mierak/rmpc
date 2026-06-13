use std::fmt;

pub struct KvBridgeLogger {
    inner: tracing_log::LogTracer,
}

impl KvBridgeLogger {
    pub fn new() -> Self {
        Self { inner: tracing_log::LogTracer::new() }
    }
}

struct KvDisplay<S>(S);

struct Visitor<'a, 'b>(&'a mut fmt::Formatter<'b>);

impl<'kvs> log::kv::VisitSource<'kvs> for Visitor<'_, '_> {
    fn visit_pair(
        &mut self,
        k: log::kv::Key<'kvs>,
        v: log::kv::Value<'kvs>,
    ) -> Result<(), log::kv::Error> {
        let _ = write!(self.0, " {k}={v}");
        Ok(())
    }
}

impl<S: log::kv::Source> fmt::Display for KvDisplay<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = self.0.visit(&mut Visitor(f));
        Ok(())
    }
}

impl log::Log for KvBridgeLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        self.inner.enabled(metadata)
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let kv = record.key_values();
        let full_msg = format_args!("{}{}", record.args(), KvDisplay(kv));
        let new_record = log::Record::builder()
            .args(full_msg)
            .level(record.level())
            .target(record.target())
            .file_static(record.file_static())
            .module_path_static(record.module_path_static())
            .line(record.line())
            .build();
        self.inner.log(&new_record);
    }

    fn flush(&self) {
        self.inner.flush();
    }
}
