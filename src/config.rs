use clap::Parser;
use tracing::Level;

#[derive(Parser, Debug)]
pub struct Config {
    #[arg(short, long, default_value = "127.0.0.1:6600")]
    pub mpd_address: String,
    #[arg(short, long, default_value_t = Level::DEBUG)]
    pub log: Level,
}
