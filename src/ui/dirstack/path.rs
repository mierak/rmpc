use derive_more::{Deref, DerefMut};

#[derive(Debug, Default, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Deref, DerefMut)]
// TODO make priv
pub struct Path(pub Vec<String>);

impl Path {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, segment: impl Into<String>) {
        self.0.push(segment.into());
    }

    pub fn pop(&mut self) -> Option<String> {
        self.0.pop()
    }

    pub fn join(&self, segment: impl Into<String>) -> Path {
        let mut res = self.0.clone();
        res.push(segment.into());
        Self(res)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.join("/"))
    }
}

impl From<String> for Path {
    fn from(value: String) -> Self {
        let mut res = Path::new();
        res.push(value);
        res
    }
}

impl From<&String> for Path {
    fn from(value: &String) -> Self {
        let mut res = Path::new();
        res.push(value);
        res
    }
}

impl From<&str> for Path {
    fn from(value: &str) -> Self {
        let mut res = Path::new();
        res.push(value);
        res
    }
}

impl From<Vec<String>> for Path {
    fn from(value: Vec<String>) -> Self {
        let mut res = Path::new();
        for val in value {
            res.push(val);
        }
        res
    }
}

impl From<&[&str]> for Path {
    fn from(value: &[&str]) -> Self {
        let mut res = Path::new();
        for val in value {
            res.push(val.to_owned());
        }
        res
    }
}

impl<const N: usize> From<[&str; N]> for Path {
    fn from(value: [&str; N]) -> Self {
        let mut res = Path::new();
        for val in value {
            res.push(val.to_owned());
        }
        res
    }
}
