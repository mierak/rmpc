use std::ops::{Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ExitCode(u8);

impl BitOr for ExitCode {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        ExitCode(self.0 | rhs.0)
    }
}

impl BitOrAssign for ExitCode {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for ExitCode {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        ExitCode(self.0 & rhs.0)
    }
}

impl BitAndAssign for ExitCode {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Add for ExitCode {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        ExitCode(self.0 + rhs.0)
    }
}

impl AddAssign for ExitCode {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl From<u8> for ExitCode {
    fn from(value: u8) -> Self {
        ExitCode(value)
    }
}

impl From<ExitCode> for i32 {
    fn from(value: ExitCode) -> Self {
        value.0 as i32
    }
}

#[cfg(test)]
mod test {
    use rstest::rstest;

    use super::ExitCode;

    #[rstest]
    #[case(0b0000_0001, 0b0000_0000, 0b0000_0000)]
    #[case(0b0000_0001, 0b0000_0001, 0b0000_0001)]
    #[case(0b0000_0010, 0b0000_0001, 0b0000_0000)]
    #[case(0b0000_0010, 0b0000_0010, 0b0000_0010)]
    fn bit_and(#[case] a: u8, #[case] b: u8, #[case] expected: u8) {
        assert_eq!(ExitCode::from(a) & ExitCode::from(b), ExitCode::from(expected));
    }

    #[rstest]
    #[case(0b0000_0001, 0b0000_0000, 0b0000_0000)]
    #[case(0b0000_0001, 0b0000_0001, 0b0000_0001)]
    #[case(0b0000_0010, 0b0000_0001, 0b0000_0000)]
    #[case(0b0000_0010, 0b0000_0010, 0b0000_0010)]
    fn bit_and_assign(#[case] a: u8, #[case] b: u8, #[case] expected: u8) {
        let mut code_a = ExitCode::from(a);
        code_a &= ExitCode::from(b);
        assert_eq!(code_a, ExitCode::from(expected));
    }

    #[rstest]
    #[case(0b0000_0001, 0b0000_0000, 0b0000_0001)]
    #[case(0b0000_0001, 0b0000_0001, 0b0000_0001)]
    #[case(0b0000_0010, 0b0000_0001, 0b0000_0011)]
    #[case(0b0000_0010, 0b0000_0010, 0b0000_0010)]
    fn bit_or(#[case] a: u8, #[case] b: u8, #[case] expected: u8) {
        assert_eq!(ExitCode::from(a) | ExitCode::from(b), ExitCode::from(expected));
    }

    #[rstest]
    #[case(0b0000_0001, 0b0000_0000, 0b0000_0001)]
    #[case(0b0000_0001, 0b0000_0001, 0b0000_0001)]
    #[case(0b0000_0010, 0b0000_0001, 0b0000_0011)]
    #[case(0b0000_0010, 0b0000_0010, 0b0000_0010)]
    fn bit_or_assign(#[case] a: u8, #[case] b: u8, #[case] expected: u8) {
        let mut code_a = ExitCode::from(a);
        code_a |= ExitCode::from(b);
        assert_eq!(code_a, ExitCode::from(expected));
    }

    #[test]
    fn add() {
        use super::ExitCode;

        let code1 = ExitCode::from(0);
        let code2 = ExitCode::from(1);
        let code3 = ExitCode::from(2);

        assert_eq!(code1 + code1, ExitCode::from(0));
        assert_eq!(code2 + code2, ExitCode::from(2));
        assert_eq!(code3 + code3, ExitCode::from(4));
        assert_eq!(code1 + code2, ExitCode::from(1));
        assert_eq!(code1 + code3, ExitCode::from(2));
        assert_eq!(code2 + code3, ExitCode::from(3));
    }
}
