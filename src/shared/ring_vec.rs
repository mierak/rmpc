use std::collections::VecDeque;

#[derive(Debug)]
pub struct RingVec<const LEN: usize, T> {
    inner: VecDeque<T>,
}

impl<const LEN: usize, T> Default for RingVec<LEN, T> {
    fn default() -> Self {
        Self { inner: VecDeque::default() }
    }
}

impl<const LEN: usize, T> RingVec<LEN, T> {
    pub fn new(input: impl Into<VecDeque<T>>) -> Self {
        Self { inner: input.into() }
    }

    pub fn push(&mut self, item: T) {
        self.inner.push_back(item);
        if self.inner.len() > LEN {
            self.inner.pop_front();
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.inner.iter()
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn last(&self) -> Option<&T> {
        self.inner.back()
    }
}

impl<const LEN: usize, T> IntoIterator for RingVec<LEN, T> {
    type IntoIter = std::collections::vec_deque::IntoIter<T>;
    type Item = T;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl<const LEN: usize, T> From<Vec<T>> for RingVec<LEN, T> {
    fn from(vec: Vec<T>) -> Self {
        Self { inner: VecDeque::from(vec) }
    }
}

impl<const LEN: usize, T> From<RingVec<LEN, T>> for Vec<T> {
    fn from(buffer: RingVec<LEN, T>) -> Self {
        buffer.inner.into()
    }
}

impl<const LEN: usize, T> From<VecDeque<T>> for RingVec<LEN, T> {
    fn from(vec: VecDeque<T>) -> Self {
        Self { inner: vec }
    }
}

impl<const LEN: usize, T> From<RingVec<LEN, T>> for VecDeque<T> {
    fn from(buffer: RingVec<LEN, T>) -> Self {
        buffer.inner
    }
}

#[cfg(test)]
mod tests {
    use super::RingVec;

    #[test]
    fn removes_last_element_when_full() {
        let mut input = RingVec::<3, usize>::from(vec![1, 2, 3]);

        input.push(4);

        assert_eq!(Vec::<_>::from(input), vec![2, 3, 4]);
    }
}
