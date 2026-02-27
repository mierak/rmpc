use std::time::Instant;

const DEFAULT_LOADING_CHARS: [&str; 8] = ["⣻", "⣽", "⣾", "⣷", "⣯", "⣟", "⡿", "⢿"];

#[derive(Debug)]
pub struct ScanStatus {
    update_start: Option<Instant>,
}

#[allow(dead_code)]
impl ScanStatus {
    pub fn new(update_start: Option<Instant>) -> Self {
        Self { update_start }
    }

    /// get updating symbol, this symbol rotates in set interval if the db is
    /// scanning
    pub fn get_str(&mut self) -> Option<&str> {
        let start = self.update_start?;
        let elapsed_secs = start.elapsed().as_millis() as usize / 250;
        let t =
            DEFAULT_LOADING_CHARS.get(elapsed_secs % DEFAULT_LOADING_CHARS.len()).unwrap_or(&"");
        Some(t)
    }
}
