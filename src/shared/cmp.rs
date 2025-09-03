use bon::Builder;
use string_compare_builder::{IsUnset, SetIgnoreLeadingTheInA, SetIgnoreLeadingTheInB, State};
use unicase::UniCase;

#[derive(Debug, Default, Builder)]
pub struct StringCompare {
    #[builder(default)]
    fold_case: bool,
    #[builder(default)]
    ignore_leading_the_in_a: bool,
    #[builder(default)]
    ignore_leading_the_in_b: bool,
}

impl<S: State> StringCompareBuilder<S> {
    pub fn ignore_leading_the(
        self,
        value: bool,
    ) -> StringCompareBuilder<SetIgnoreLeadingTheInB<SetIgnoreLeadingTheInA<S>>>
    where
        S::IgnoreLeadingTheInA: IsUnset,
        S::IgnoreLeadingTheInB: IsUnset,
    {
        self.ignore_leading_the_in_a(value).ignore_leading_the_in_b(value)
    }
}

fn strip_the(input: &str) -> &str {
    if input.len() < 4 {
        return input;
    }

    let Some(s) = input.get(..4) else {
        return input;
    };

    if s == "THE " || s == "the " || s == "The " {
        return &input[4..];
    }

    return input;
}

#[allow(unused)]
impl StringCompare {
    pub fn compare(&self, a: &str, b: &str) -> std::cmp::Ordering {
        let a = if self.ignore_leading_the_in_a { strip_the(a) } else { a };
        let b = if self.ignore_leading_the_in_b { strip_the(b) } else { b };

        if self.fold_case { UniCase::new(a).cmp(&UniCase::new(b)) } else { a.cmp(b) }
    }

    pub fn eq(&self, a: &str, b: &str) -> bool {
        let a = if self.ignore_leading_the_in_a { strip_the(a) } else { a };
        let b = if self.ignore_leading_the_in_b { strip_the(b) } else { b };

        if self.fold_case { UniCase::new(a) == UniCase::new(b) } else { a == b }
    }
}

#[cfg(test)]
mod test {
    use rstest::rstest;

    use crate::shared::cmp::StringCompare;

    #[rstest]
    #[case("", "")]
    #[case("a", "a")]
    #[case("A", "a")]
    #[case("soMerAnDomStuFff", "somerandomstufff")]
    fn fold_case_ordering_equal_fold_case(#[case] a: &str, #[case] b: &str) {
        let cmp = StringCompare::builder().fold_case(true).build();

        assert_eq!(cmp.compare(a, b), std::cmp::Ordering::Equal);
    }

    #[rstest]
    #[case("a", "b")]
    #[case("A", "b")]
    #[case("soMerAnDomStuFff", "somerandomstuffg")]
    pub fn fold_case_ordering_less_fold_case(#[case] a: &str, #[case] b: &str) {
        let cmp = StringCompare::builder().fold_case(true).build();

        assert_eq!(cmp.compare(a, b), std::cmp::Ordering::Less);
    }

    #[rstest]
    #[case("b", "a")]
    #[case("B", "a")]
    #[case("somerandomstuffg", "soMerAnDomStuFff")]
    pub fn fold_case_ordering_greater_fold_case(#[case] a: &str, #[case] b: &str) {
        let cmp = StringCompare::builder().fold_case(true).build();

        assert_eq!(cmp.compare(a, b), std::cmp::Ordering::Greater);
    }

    #[rstest]
    #[case("", "")]
    #[case("a", "a")]
    #[case("A", "A")]
    #[case("soMerAnDomStuFff", "soMerAnDomStuFff")]
    fn no_fold_case_ordering_equal(#[case] a: &str, #[case] b: &str) {
        let cmp = StringCompare::builder().fold_case(false).build();

        assert_eq!(cmp.compare(a, b), std::cmp::Ordering::Equal);
    }

    #[rstest]
    #[case("a", "b")]
    #[case("A", "B")]
    #[case("A", "a")]
    #[case("soMerAnDomStuFff", "soMerAnDomStuFfg")]
    pub fn no_fold_case_ordering_less(#[case] a: &str, #[case] b: &str) {
        let cmp = StringCompare::builder().fold_case(false).build();

        assert_eq!(cmp.compare(a, b), std::cmp::Ordering::Less);
    }

    #[rstest]
    #[case("b", "a")]
    #[case("B", "A")]
    #[case("a", "A")]
    pub fn no_fold_case_ordering_greater(#[case] a: &str, #[case] b: &str) {
        let cmp = StringCompare::builder().fold_case(false).build();

        assert_eq!(cmp.compare(a, b), std::cmp::Ordering::Greater);
    }

    #[rstest]
    #[case("The Beatles", "Beatles")]
    #[case("the Beatles", "Beatles")]
    #[case("THE Beatles", "Beatles")]
    #[case("The Beatles", "THE Beatles")]
    #[case("the Beatles", "the Beatles")]
    #[case("THE Beatles", "the Beatles")]
    fn ignore_leading_the_equal(#[case] a: &str, #[case] b: &str) {
        let cmp = StringCompare::builder().ignore_leading_the(true).build();

        assert_eq!(cmp.compare(a, b), std::cmp::Ordering::Equal);
    }

    #[rstest]
    #[case("The Beatles", "the Beatles", true)]
    #[case("The Beatles", "Beatles", true)]
    #[case("Beatles", "Beatles the", false)]
    #[case("Beatles", "Beatles", true)]
    #[case("BEATLES", "beatles", true)]
    #[case("THE BEATLES", "beatles", true)]
    #[case("THE BEATLES", "THE beatles", true)]
    #[case("THE BEATLES", "the beatles", true)]
    fn eq_ignore_case_and_leading_the(#[case] a: &str, #[case] b: &str, #[case] expected: bool) {
        let cmp = StringCompare::builder().ignore_leading_the(true).fold_case(true).build();

        assert_eq!(cmp.eq(a, b), expected);
    }
}
