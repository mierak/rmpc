use anyhow::{anyhow, Context, Result};

#[derive(Debug)]
pub struct MpdList(pub Vec<String>);

impl std::str::FromStr for MpdList {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(MpdList(
            s.lines()
                .map(|line| -> Result<String> {
                    Ok(line
                        .split_once(": ")
                        .context(anyhow!("Unable to split value: '{}'", line))?
                        .1
                        .to_owned())
                })
                .collect::<Result<Vec<String>>>()?,
        ))
    }
}
