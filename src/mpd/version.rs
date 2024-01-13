use std::{fmt::Display, str::FromStr};

use super::errors::MpdError;

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl Version {
    pub fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self { major, minor, patch }
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for Version {
    type Err = MpdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.trim().split('.');
        let major = parts
            .next()
            .ok_or(MpdError::Parse(format!("Cannot parse major version from '{s}'")))?;
        let minor = parts
            .next()
            .ok_or(MpdError::Parse(format!("Cannot parse minor version from '{s}'")))?;
        let patch = parts
            .next()
            .ok_or(MpdError::Parse(format!("Cannot parse patch version from '{s}'")))?;
        Ok(Self {
            major: major.parse()?,
            minor: minor.parse()?,
            patch: patch.parse()?,
        })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::mpd::version::Version;

    #[test]
    fn test_version_eq() {
        let version_1: Version = "0.22.0".parse().unwrap();
        let version_2: Version = "0.22.0".parse().unwrap();

        assert!(version_1 == version_2);
    }

    #[test]
    fn test_version_major() {
        let version_1: Version = "1.23.0".parse().unwrap();
        let version_2: Version = "0.28.0".parse().unwrap();

        assert!(version_1 > version_2);
    }

    #[test]
    fn test_version_minor() {
        let version_1: Version = "0.23.0".parse().unwrap();
        let version_2: Version = "0.22.0".parse().unwrap();

        assert!(version_1 > version_2);
    }

    #[test]
    fn test_version_patch() {
        let version_1: Version = "0.22.1".parse().unwrap();
        let version_2: Version = "0.22.0".parse().unwrap();

        assert!(version_1 > version_2);
    }
}
