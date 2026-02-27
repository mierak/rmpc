#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MpdAddress {
    IpAndPort(String),
    SocketPath(String),
    AbstractSocket(String),
}

#[derive(Default, Clone, Eq, PartialEq)]
pub struct MpdPassword(pub String);
impl std::fmt::Debug for MpdPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "*****")
    }
}

impl From<&str> for MpdPassword {
    fn from(s: &str) -> Self {
        s.to_owned().into()
    }
}

impl From<String> for MpdPassword {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl Default for MpdAddress {
    fn default() -> Self {
        Self::IpAndPort("127.0.0.1:6600".to_string())
    }
}
