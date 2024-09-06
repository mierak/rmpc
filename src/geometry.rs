use ratatui::layout::Direction;

use crate::utils::percent::Percent;

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub struct Point {
    pub x: u16,
    pub y: u16,
}

impl Point {
    pub fn distance(self, other: Self) -> u16 {
        self.x.abs_diff(other.x) + self.y.abs_diff(other.y)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Geometry {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    taken_size_horiz: u16,
    taken_size_vert: u16,
}

impl Geometry {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
            taken_size_horiz: 0,
            taken_size_vert: 0,
        }
    }

    pub fn top_left_dist(&self, other: Self) -> u16 {
        Point { x: self.x, y: self.y }.distance(Point { x: other.x, y: other.y })
    }

    pub fn take_chunk(&mut self, direction: Direction, size: Percent) -> Geometry {
        match direction {
            Direction::Horizontal => self.take_chunk_horiz(size),
            Direction::Vertical => self.take_chunk_vert(size),
        }
    }

    pub fn middle(self) -> Point {
        Point {
            x: self.x + self.width / 2,
            y: self.y + self.height / 2,
        }
    }

    pub fn top_left(self) -> Point {
        Point { x: self.x, y: self.y }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn take_chunk_horiz(&mut self, size_percent: Percent) -> Geometry {
        let size = ((u32::from(self.width) * 100) * u32::from(size_percent) / 10000) as u16;

        let result = Geometry::new(self.x + self.taken_size_horiz, self.y, size, self.height);
        self.taken_size_horiz += size;

        result
    }

    #[allow(clippy::cast_possible_truncation)]
    fn take_chunk_vert(&mut self, size_percent: Percent) -> Geometry {
        let size = ((u32::from(self.height) * 100) * u32::from(size_percent) / 10000) as u16;

        let result = Geometry::new(self.x, self.y + self.taken_size_vert, self.width, size);
        self.taken_size_vert += size;

        result
    }

    pub fn take_remainder(&mut self) -> Geometry {
        Geometry::new(
            self.x + self.taken_size_horiz,
            self.y + self.taken_size_vert,
            self.width - self.taken_size_horiz,
            self.height - self.taken_size_vert,
        )
    }

    pub fn is_directly_above(&self, other: Self) -> bool {
        if !(self.x + self.width > other.x && other.x + other.width > self.x) {
            return false;
        }

        if other.middle().y < self.middle().y {
            return false;
        }

        true
    }

    pub fn is_directly_below(&self, other: Self) -> bool {
        if !(self.x + self.width > other.x && other.x + other.width > self.x) {
            return false;
        }

        if other.middle().y > self.middle().y {
            return false;
        }

        true
    }

    pub fn is_directly_right(&self, other: Self) -> bool {
        if !(self.y + self.height > other.y && other.y + other.height > self.y) {
            return false;
        }

        if other.middle().x > self.middle().x {
            return false;
        }

        true
    }

    pub fn is_directly_left(&self, other: Self) -> bool {
        if !(self.y + self.height > other.y && other.y + other.height > self.y) {
            return false;
        }

        if other.middle().x < self.middle().x {
            return false;
        }

        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use ratatui::layout::Direction;
    use test_case::test_case;

    use super::Geometry;

    #[test_case(100, 40, 40)]
    fn test(input: u16, chunk: u16, output: u16) {
        let mut input = Geometry::new(0, 0, input, 100);

        assert_eq!(
            input
                .take_chunk(Direction::Horizontal, format!("{chunk}%").parse().unwrap())
                .width,
            output
        );
    }
}
