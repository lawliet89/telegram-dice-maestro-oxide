use std::str::FromStr;

use nom::{
    character::complete::multispace0,
    character::complete::one_of,
    combinator::consumed,
    error::ParseError,
    multi::{many1, many_m_n},
    sequence::delimited,
    sequence::Tuple,
    Finish, IResult,
};
use thiserror::Error;

use crate::dice::RollSettings;

/// A combinator that takes a parser `inner` and produces a parser that also consumes both leading and
/// trailing whitespace, returning the output of `inner`.
/// https://docs.rs/nom/latest/nom/recipes/index.html#wrapper-combinators-that-eat-whitespace-before-and-after-a-parser
fn ws<'a, F: 'a, O, E: ParseError<&'a str>>(
    inner: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: FnMut(&'a str) -> IResult<&'a str, O, E>,
{
    delimited(multispace0, inner, multispace0)
}

fn single_decimal(input: &str) -> IResult<&str, char> {
    ws(one_of("0123456789"))(input)
}

fn decimal<T>(input: &str, min: usize, max: usize) -> IResult<&str, T>
where
    T: FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    let (remainning, (_, chars)) = consumed(many_m_n(min, max, single_decimal))(input)?;
    let output = String::from_iter(chars);
    Ok((remainning, output.parse().expect("parsing to succeed")))
}

fn dice_seperator(input: &str) -> IResult<&str, char> {
    let (remaining, (_, separator)) = ws(consumed(one_of("dD")))(input)?;
    Ok((remaining, separator))
}

fn modifier_separator(input: &str) -> IResult<&str, char> {
    let (remaining, (_, separator)) = ws(consumed(one_of("+-")))(input)?;
    Ok((remaining, separator))
}

#[derive(Error, Debug, PartialEq)]
pub(crate) enum ParseRollError {
    #[error("Error parsing input: {0}")]
    ParseError(String),
    #[error("Number of dices and dice sides cannot be zero: {0}")]
    CannotBeZero(String),
    #[error("Input parameter is too big")]
    TooBig,
}

impl<'a> From<nom::error::Error<&'a str>> for ParseRollError {
    fn from(e: nom::error::Error<&str>) -> Self {
        ParseRollError::ParseError(e.to_string())
    }
}

fn parse_roll_inner(input: &str) -> IResult<&str, RollSettings> {
    let digits = |i| decimal::<u32>(i, 1, 4);

    log::debug!("Parsing input: {}", input);
    let (remaining, (number, _, sides)) = (&digits, &dice_seperator, &digits).parse(input)?;
    log::debug!("Parsed Sides: {:?}", sides);
    log::debug!("Parsed Number: {:?}", number);
    log::debug!("Parsed Remaining: {}", remaining);

    let modifier_parse = (&modifier_separator, &digits).parse(remaining);

    let (remaining, modifier) = match modifier_parse {
        Ok((remaining, (modifier_sign, modifier))) => {
            log::debug!("Modifier Sign: {:?}", modifier_sign);
            log::debug!("Modifier: {:?}", modifier);
            log::debug!("Remaining: {}", remaining);
            // Parse modifier
            let modifier = format!("{}{}", modifier_sign, modifier)
                .parse::<i32>()
                .expect("to parse");

            (remaining, Some(modifier))
        }
        Err(_) => (remaining, None),
    };

    Ok((
        remaining,
        RollSettings {
            sides,
            number,
            modifier,
            label: None,
        },
    ))
}

pub(crate) fn parse_roll(input: &str) -> Result<RollSettings, ParseRollError> {
    let (remaining, mut result) = parse_roll_inner(input).finish()?;

    if result.number == 0 || result.sides == 0 {
        Err(ParseRollError::CannotBeZero(input.to_string()))?
    }

    // Check remaining text is not "overflow" digits
    if consumed(consumed(many1(single_decimal)))(remaining).is_ok() {
        Err(ParseRollError::TooBig)?;
    }

    let remaining = remaining.trim();
    if !remaining.is_empty() {
        result.label = Some(remaining.to_string());
    }

    Ok(result)
}

// pub(crate) fn parse_roll<S>(input: S) -> Result<(u32, u32, Option<i32>), ParseRollError>
// where
//     S: AsRef<str> + std::fmt::Display + std::ops::Deref<Target = str>,
// {
//     lazy_static! {
//         static ref RE: Regex =
//             Regex::new(r"^([0-9]{1,4})(d|D)([0-9]{1,4})([+-][0-9]{1,4})?$").unwrap();
//     }
//     log::trace!("Cleaning raw input {}", &input);
//     let stripped = input.trim();
//     log::info!("Parsing input {}", &input);
//     let captures = RE.captures(stripped).ok_or_else(|| {
//         log::warn!("Regex match failure for {}", &input);
//         ParseRollError::ParseError(stripped.to_string())
//     })?;

//     // 1d20+4
//     // Some(Captures({
//     //     0: Some("1d20+4"),
//     //     1: Some("1"),
//     //     2: Some("d"),
//     //     3: Some("20"),
//     //     4: Some("+4"),
//     // })),
//     let number = captures
//         .get(1)
//         .expect("to exist")
//         .as_str()
//         .parse::<u32>()
//         .expect("to be integer");
//     let sides = captures
//         .get(3)
//         .expect("to exist")
//         .as_str()
//         .parse::<u32>()
//         .expect("to be integer");
//     let modifier = captures
//         .get(4)
//         .map(|res| res.as_str().parse::<i32>().expect("to be integer"));

//     if number == 0 || sides == 0 {
//         Err(ParseRollError::CannotBeZero(stripped.to_string()))?
//     }

//     Ok((number, sides, modifier))
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dice::*;

    #[test]
    fn single_decimal_parses_correctly() {
        let cases = [("1", '1'), (" 1", '1'), ("1  ", '1'), ("   1   ", '1')];
        for (input, expected) in cases {
            // println!("Test case: {:?} Expected {:?}", input, expected);
            let (remaining, actual) = single_decimal(input).finish().unwrap();
            assert_eq!(actual, expected);
            assert!(remaining.is_empty());
        }
    }

    #[test]
    fn decimal_parses_correctly() {
        let cases = [
            ("123456", 123456),
            (" 123456", 123456),
            ("123456 ", 123456),
            ("1 234 56", 123456),
        ];
        for (input, expected) in cases {
            // println!("Test case: {:?} Expected {:?}", input, expected);
            let (remaining, actual): (_, u32) = decimal(input, 1, 10).finish().unwrap();
            assert_eq!(actual, expected);
            assert!(remaining.is_empty());
        }
    }

    #[test]
    fn parses_correctly() {
        let cases = [
            (
                "1d20",
                Ok(RollSettings {
                    number: 1,
                    sides: 20,
                    modifier: None,
                    label: None,
                }),
            ),
            (
                "1d20+3",
                Ok(RollSettings {
                    number: 1,
                    sides: 20,
                    modifier: Some(3),
                    label: None,
                }),
            ),
            (
                "1d20-2",
                Ok(RollSettings {
                    number: 1,
                    sides: 20,
                    modifier: Some(-2),
                    label: None,
                }),
            ),
            (
                "1 d 20",
                Ok(RollSettings {
                    number: 1,
                    sides: 20,
                    modifier: None,
                    label: None,
                }),
            ),
            (
                "1d20 - 2",
                Ok(RollSettings {
                    number: 1,
                    sides: 20,
                    modifier: Some(-2),
                    label: None,
                }),
            ),
            (
                "1d20 Wisdom saving throw",
                Ok(RollSettings {
                    number: 1,
                    sides: 20,
                    modifier: None,
                    label: Some("Wisdom saving throw".to_string()),
                }),
            ),
            (
                "1d20Wisdom saving throw",
                Ok(RollSettings {
                    number: 1,
                    sides: 20,
                    modifier: None,
                    label: Some("Wisdom saving throw".to_string()),
                }),
            ),
            (
                "1d20+1 Wisdom saving throw",
                Ok(RollSettings {
                    number: 1,
                    sides: 20,
                    modifier: Some(1),
                    label: Some("Wisdom saving throw".to_string()),
                }),
            ),
            (
                "1d20+1Wisdom saving throw",
                Ok(RollSettings {
                    number: 1,
                    sides: 20,
                    modifier: Some(1),
                    label: Some("Wisdom saving throw".to_string()),
                }),
            ),
            (
                "9999d9999+3",
                Ok(RollSettings {
                    number: 9999,
                    sides: 9999,
                    modifier: Some(3),
                    label: None,
                }),
            ),
            // too many dices
            (
                "100000d20",
                Err(ParseRollError::ParseError(
                    "error OneOf at: 00d20".to_string(),
                )),
            ),
            // too many sides
            ("1d100000", Err(ParseRollError::TooBig)),
            // modifier too big
            ("1d20+10000000", Err(ParseRollError::TooBig)),
            // zero dice
            (
                "0d20+2",
                Err(ParseRollError::CannotBeZero("0d20+2".to_string())),
            ),
            // zero sided dice
            (
                "1d0+2",
                Err(ParseRollError::CannotBeZero("1d0+2".to_string())),
            ),
            (
                "rubbish",
                Err(ParseRollError::ParseError(
                    "error OneOf at: rubbish".to_string(),
                )),
            ),
        ];

        for (input, expected) in cases {
            // println!("Test case: {:?} Expected {:?}", input, expected);
            let actual = RollSettings::from_str(input);
            match expected {
                Ok(expected) => {
                    assert_eq!(expected, actual.unwrap());
                }
                Err(e) => {
                    assert_eq!(e, actual.unwrap_err());
                }
            }
        }
    }
}
