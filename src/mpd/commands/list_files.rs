use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;

// file: 03 Diode.flac
// size: 18183774
// Last-Modified: 2022-12-24T13:02:09Z
#[derive(Debug, Default)]
pub struct ListFiles(Vec<Listed>);
#[derive(Debug, Default)]
pub struct Listed {
    pub kind: ListingType,
    pub name: String,
    pub size: u64,
    pub last_modified: String, // TODO timestamp?
}

#[allow(dead_code)]
impl ListFiles {
    pub fn value(&self) -> &Vec<Listed> {
        &self.0
    }

    pub fn value_mut(&mut self) -> &mut Vec<Listed> {
        &mut self.0
    }
}

#[derive(Debug, Default)]
pub enum ListingType {
    #[default]
    File,
    Dir,
}

impl std::str::FromStr for ListFiles {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut acc: Vec<Listed> = Vec::new();

        let mut current = String::new();
        for (i, line) in s.lines().enumerate() {
            if line.starts_with("file:") || line.starts_with("directory:") {
                if i > 0 {
                    acc.push(current.parse()?);
                }
                current = String::new();
            }

            current.push_str(line);
            current.push('\n');
        }
        acc.push(current.parse()?);

        Ok(Self(acc))
    }
}

impl std::str::FromStr for Listed {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut lines = s.lines();
        let mut val = Listed::default();
        if let Some(line) = lines.next() {
            let (key, value) = line
                .split_once(": ")
                .context(anyhow!("Invalid value '{}' whe parsing Dir or File for Listed", line))?;
            if key == "file" {
                val.kind = ListingType::File;
            } else if key == "directory" {
                val.kind = ListingType::Dir;
            }
            val.name = value.to_owned();

            for s in lines {
                let (key, value) = s
                    .split_once(": ")
                    .context(anyhow!("Invalid value '{}' whe parsing ListedFile", line))?;
                match key {
                    "size" => val.size = value.parse()?,
                    "Last-Modified" => val.last_modified = value.to_owned(),
                    key => {
                        tracing::warn!(
                            message = "Encountered unknow key/value pair while parsing 'listfiles' command",
                            key,
                            value
                        );
                    }
                }
            }

            Ok(val)
        } else {
            Err(anyhow!("Invalid value. Cannot parse Listed. '{}'", s))
        }
    }
}
