pub(crate) trait TimeProvider {
    fn now(&self) -> std::time::Instant;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DefaultTimeProvider;
impl TimeProvider for DefaultTimeProvider {
    fn now(&self) -> std::time::Instant {
        std::time::Instant::now()
    }
}
