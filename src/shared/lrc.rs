use std::{collections::HashMap, str::FromStr, time::Duration};

use anyhow::{bail, Context};

#[derive(Debug, Eq, PartialEq)]
pub struct LrcLine {
    pub time: Duration,
    pub content: String,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Lrc {
    pub lines: Vec<LrcLine>,
    pub metadata: HashMap<String, String>,
}

impl FromStr for Lrc {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut lines = Vec::new();
        let mut metadata = HashMap::new();

        for s in s.lines() {
            if s.is_empty() {
                continue;
            }

            let (meta_or_time, line) = s
                .trim()
                .strip_prefix('[')
                .and_then(|s| s.split_once(']'))
                .with_context(|| format!("Invalid lrc line format: '{s}'"))?;

            match meta_or_time.chars().next() {
                Some(c) if c.is_numeric() => {
                    let (minutes, time_rest) = meta_or_time
                        .split_once(':')
                        .with_context(|| format!("Invalid lrc minutes format: '{meta_or_time}'"))?;
                    let (seconds, hundreths) = time_rest
                        .split_once('.')
                        .or_else(|| time_rest.split_once(':'))
                        .with_context(|| format!("Invalid lrc seconds and hundreths format: '{time_rest}'"))?;

                    let mut milis = 0;
                    milis += minutes.parse::<u64>()? * 60 * 1000;
                    milis += seconds.parse::<u64>()? * 1000;
                    milis += hundreths.parse::<u64>()? * 10;

                    lines.push(LrcLine {
                        time: Duration::from_millis(milis),
                        content: line.to_owned(),
                    });
                }
                Some(_) => {
                    let (key, value) = meta_or_time
                        .split_once(':')
                        .with_context(|| format!("Invalid metadata line: '{meta_or_time}'"))?;
                    metadata.insert(key.trim().to_string(), value.trim().to_string());
                }
                None => {
                    bail!("Invalid lrc metadata/timestamp: '{meta_or_time}'");
                }
            }
        }

        Ok(Self { lines, metadata })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::{collections::HashMap, time::Duration};

    use crate::shared::lrc::{Lrc, LrcLine};

    #[test]
    fn lrc() {
        let input = r"[t1: asdf ]
[t2:123]
[length: 2:23]
[offset: -15000]

[00:01.86]line with dot before hundredths
[00:04.73]line with colon before hundredths
[00:11.24]
[11:16.91]line with long time";

        let result: Lrc = input.parse().unwrap();

        assert_eq!(
            result,
            Lrc {
                lines: vec![
                    LrcLine {
                        time: Duration::from_millis(1860),
                        content: "line with dot before hundredths".to_string()
                    },
                    LrcLine {
                        time: Duration::from_millis(4730),
                        content: "line with colon before hundredths".to_string()
                    },
                    LrcLine {
                        time: Duration::from_millis(11240),
                        content: String::new()
                    },
                    LrcLine {
                        time: Duration::from_millis(676_910),
                        content: "line with long time".to_string()
                    },
                ],
                metadata: [("t1", "asdf"), ("t2", "123"), ("length", "2:23"), ("offset", "-15000"),]
                    .iter()
                    .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                    .collect::<HashMap<_, _>>()
            }
        );
    }
}
