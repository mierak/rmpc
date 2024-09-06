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
        let res = Geometry::new(
            self.x + self.taken_size_horiz,
            self.y + self.taken_size_vert,
            self.width - self.taken_size_horiz,
            self.height - self.taken_size_vert,
        );
        self.taken_size_horiz = self.width;
        self.taken_size_vert = self.height;

        res
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
#[rustfmt::skip]
mod tests {
    use ratatui::layout::Direction;
    use test_case::test_case;

    use super::Geometry;

    //                         x   y   w   h                  x   y    w    h
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0,  0,  25,  10), false; "above, left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(75,  0,  25,  10), false; "above, right")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 25,  10,  25), false; "left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 75,  10,  25), false; "right")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(25,  0,  50,  10), true ; "above")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(25, 75,  50,  10), false; "below")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 75,  25,  10), false; "below, left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(75, 75,  25,  10), false; "below, right")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0,  0, 100,  10), true ; "whole space above")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0,  0,  26,  10), true ; "partial from left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0,  0,  26,  10), true ; "partial from right")]
    fn is_above(g1: Geometry, g2: Geometry, overlaps: bool) {
        assert_eq!(g2.is_directly_above(g1), overlaps);
    }

    //                         x   y   w   h                  x   y    w    h
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0,  0,  25,  10), false; "above, left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(75,  0,  25,  10), false; "above, right")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 25,  10,  25), false; "left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 75,  10,  25), false; "right")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(25,  0,  50,  10), false; "above")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(25, 75,  50,  10), true ; "below")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 75,  25,  10), false; "below, left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(75, 75,  25,  10), false; "below, right")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 80, 100,  10), true ; "whole space below")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 80,  26,  10), true ; "partial from left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 80,  26,  10), true ; "partial from right")]
    fn is_below(g1: Geometry, g2: Geometry, overlaps: bool) {
        assert_eq!(g2.is_directly_below(g1), overlaps);
    }

    //                         x   y   w   h                  x   y    w    h
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0,  0,  25,  10), false; "above, left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(75,  0,  25,  10), false; "above, right")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 25,  10,  25), true ; "left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(75, 25,  10,  25), false; "right")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(25,  0,  50,  10), false; "above")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(25, 75,  50,  10), false; "below")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 75,  25,  10), false; "below, left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(75, 75,  25,  10), false; "below, right")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 80, 100,  10), false; "whole space below")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 80,  26,  10), false; "partial from left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 80,  26,  10), false; "partial from right")]
    fn is_left(g1: Geometry, g2: Geometry, overlaps: bool) {
        assert_eq!(g2.is_directly_left(g1), overlaps);
    }

    //                         x   y   w   h                  x   y    w    h
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0,  0,  25,  10), false; "above, left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(75,  0,  25,  10), false; "above, right")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 25,  10,  25), false; "left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(75, 25,  10,  25), true ; "right")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(25,  0,  50,  10), false; "above")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(25, 75,  50,  10), false; "below")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 75,  25,  10), false; "below, left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new(75, 75,  25,  10), false; "below, right")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 80, 100,  10), false; "whole space below")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 80,  26,  10), false; "partial from left")]
    #[test_case(Geometry::new(25, 25, 50, 50), Geometry::new( 0, 80,  26,  10), false; "partial from right")]
    fn is_right(g1: Geometry, g2: Geometry, overlaps: bool) {
        assert_eq!(g2.is_directly_right(g1), overlaps);
    }

    #[test]
    fn take_chunk_horizontal() {
        let mut input = Geometry::new(0, 0, 100, 100);

        let c1 = input.take_chunk(Direction::Horizontal, "20%".parse().unwrap());

        assert_eq!(c1,    Geometry {x:  0, y: 0, width:  20, height: 100, taken_size_horiz:  0, taken_size_vert:   0 });
        assert_eq!(input, Geometry {x:  0, y: 0, width: 100, height: 100, taken_size_horiz: 20, taken_size_vert:   0 });

        let c2 = input.take_chunk(Direction::Horizontal, "20%".parse().unwrap());
        assert_eq!(c2,    Geometry {x: 20, y: 0, width:  20, height: 100, taken_size_horiz:  0, taken_size_vert:   0 });
        assert_eq!(input, Geometry {x:  0, y: 0, width: 100, height: 100, taken_size_horiz: 40, taken_size_vert:   0 });

        let c3 = input.take_chunk(Direction::Horizontal, "35%".parse().unwrap());
        assert_eq!(c3,    Geometry {x: 40, y: 0, width:  35, height: 100, taken_size_horiz:  0, taken_size_vert:   0 });
        assert_eq!(input, Geometry {x:  0, y: 0, width: 100, height: 100, taken_size_horiz: 75, taken_size_vert:   0 });

        let c4 = input.take_remainder();
        assert_eq!(c4,    Geometry {x: 75, y: 0, width:  25, height: 100, taken_size_horiz:   0, taken_size_vert:   0 });
        assert_eq!(input, Geometry {x:  0, y: 0, width: 100, height: 100, taken_size_horiz: 100, taken_size_vert: 100 });
    }

    #[test]
    fn take_chunk_vertical() {
        let mut input = Geometry::new(0, 0, 100, 100);

        let c1 = input.take_chunk(Direction::Vertical, "20%".parse().unwrap());

        assert_eq!(c1,    Geometry {x:  0, y: 0, width: 100, height:  20, taken_size_horiz:  0, taken_size_vert:   0 });
        assert_eq!(input, Geometry {x:  0, y: 0, width: 100, height: 100, taken_size_horiz:  0, taken_size_vert:  20 });

        let c2 = input.take_chunk(Direction::Vertical, "20%".parse().unwrap());
        assert_eq!(c2,    Geometry {x:  0, y: 20, width: 100, height:  20, taken_size_horiz:  0, taken_size_vert:   0 });
        assert_eq!(input, Geometry {x:  0, y:  0, width: 100, height: 100, taken_size_horiz:  0, taken_size_vert:  40 });

        let c3 = input.take_chunk(Direction::Vertical, "35%".parse().unwrap());
        assert_eq!(c3,    Geometry {x:  0, y: 40, width: 100, height:  35, taken_size_horiz:  0, taken_size_vert:   0 });
        assert_eq!(input, Geometry {x:  0, y:  0, width: 100, height: 100, taken_size_horiz:  0, taken_size_vert:  75 });

        let c4 = input.take_remainder();
        assert_eq!(c4,    Geometry {x:  0, y: 75, width: 100, height:  25, taken_size_horiz:   0, taken_size_vert:   0 });
        assert_eq!(input, Geometry {x:  0, y:  0, width: 100, height: 100, taken_size_horiz: 100, taken_size_vert: 100 });
    }
}
