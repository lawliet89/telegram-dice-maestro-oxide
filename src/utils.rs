use std::borrow::Cow;

pub fn strip_whitespace(input: &str) -> Cow<str> {
    if input.chars().any(|c| c.is_whitespace()) {
        let stripped: String = input.chars().filter(|c| !c.is_whitespace()).collect();
        Cow::Owned(stripped)
    } else {
        Cow::Borrowed(input)
    }
}
