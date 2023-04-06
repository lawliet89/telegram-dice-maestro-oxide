use std::str::FromStr;

use lazy_static::lazy_static;
use rand::distributions::{Distribution, Uniform};
use regex::Regex;
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub(crate) enum ParseRollError {
    #[error("Invalid roll format: {0}")]
    InvalidFormat(String),
    #[error("Number of dices and dice sides cannot be zero: {0}")]
    CannotBeZero(String),
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct RollSettings {
    pub number: u32,
    pub sides: u32,
    pub modifier: Option<i32>,
}

impl RollSettings {
    pub fn roll(&self) -> RollResults {
        RollResults::new(self)
    }
}

impl FromStr for RollSettings {
    type Err = ParseRollError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let (number, sides, modifier) = parse_roll(input)?;
        Ok(Self {
            number,
            sides,
            modifier,
        })
    }
}

#[derive(Serialize, Debug)]
pub(crate) struct RollResults<'a> {
    pub rolls: Vec<u32>,
    pub total: i64,
    pub settings: &'a RollSettings,
}

impl<'a> RollResults<'a> {
    fn new(settings: &'a RollSettings) -> Self {
        let mut rng = rand::thread_rng();
        let die = Uniform::from(1..=settings.sides);

        let rolls: Vec<u32> = (1..=settings.number)
            .map(|_| die.sample(&mut rng))
            .collect();

        let mut total: i64 = rolls.iter().map(|i| *i as i64).sum();
        if let Some(modifier) = settings.modifier {
            total += modifier as i64
        }

        RollResults {
            settings,
            rolls,
            total,
        }
    }
}

impl<'a> std::fmt::Display for RollResults<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let modifier_text = match self.settings.modifier {
            None => "".to_string(),
            Some(number) => {
                if number > 0 {
                    format!(" + {}", number)
                } else {
                    format!(" - {}", -number)
                }
            }
        };

        writeln!(
            f,
            "Parameters: {}d{}{}",
            self.settings.number, self.settings.sides, modifier_text
        )?;

        let mut results = self
            .rolls
            .iter()
            .map(ToString::to_string)
            .reduce(|a, b| format!("{} + {}", a, b))
            .expect("to not be empty");

        if results.len() > 4000 {
            results.truncate(4000);
            results.push_str("...");
        }

        // https://stackoverflow.com/questions/68768069/telegram-error-badrequest-entities-too-long-error-when-trying-to-send-long-ma
        // tldr; limit is 9500
        writeln!(f, "Roll: ({}){}", results, modifier_text)?;

        write!(f, "Your final roll is: 🎲 <b>{}</b> 🎲", self.total)
    }
}

fn parse_roll<S>(input: S) -> Result<(u32, u32, Option<i32>), ParseRollError>
where
    S: AsRef<str> + std::fmt::Display + std::ops::Deref<Target = str>,
{
    lazy_static! {
        static ref RE: Regex =
            Regex::new(r"^([0-9]{1,4})(d|D)([0-9]{1,4})([+-][0-9]{1,4})?$").unwrap();
    }
    log::trace!("Cleaning raw input {}", &input);
    let stripped: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    log::info!("Parsing input {}", &input);
    let captures = RE.captures(&stripped).ok_or_else(|| {
        log::warn!("Regex match failure for {}", &input);
        ParseRollError::InvalidFormat(stripped.clone())
    })?;

    // 1d20+4
    // Some(Captures({
    //     0: Some("1d20+4"),
    //     1: Some("1"),
    //     2: Some("d"),
    //     3: Some("20"),
    //     4: Some("+4"),
    // })),
    let number = captures
        .get(1)
        .expect("to exist")
        .as_str()
        .parse::<u32>()
        .expect("to be integer");
    let sides = captures
        .get(3)
        .expect("to exist")
        .as_str()
        .parse::<u32>()
        .expect("to be integer");
    let modifier = captures
        .get(4)
        .map(|res| res.as_str().parse::<i32>().expect("to be integer"));

    if number == 0 || sides == 0 {
        Err(ParseRollError::CannotBeZero(stripped))?
    }

    Ok((number, sides, modifier))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_correctly() {
        let cases = [
            (
                "rubbish",
                Err(ParseRollError::InvalidFormat("rubbish".to_string())),
            ),
            (
                "1d20",
                Ok(RollSettings {
                    number: 1,
                    sides: 20,
                    modifier: None,
                }),
            ),
            (
                "1d20+3",
                Ok(RollSettings {
                    number: 1,
                    sides: 20,
                    modifier: Some(3),
                }),
            ),
            (
                "1d20-2",
                Ok(RollSettings {
                    number: 1,
                    sides: 20,
                    modifier: Some(-2),
                }),
            ),
            (
                "1 d 20",
                Ok(RollSettings {
                    number: 1,
                    sides: 20,
                    modifier: None,
                }),
            ),
            (
                "1d20 - 2",
                Ok(RollSettings {
                    number: 1,
                    sides: 20,
                    modifier: Some(-2),
                }),
            ),
            (
                "9999d9999+3",
                Ok(RollSettings {
                    number: 9999,
                    sides: 9999,
                    modifier: Some(3),
                }),
            ),
            // too many dices
            (
                "100000d20",
                Err(ParseRollError::InvalidFormat("100000d20".to_string())),
            ),
            // too many sides
            (
                "1d100000",
                Err(ParseRollError::InvalidFormat("1d100000".to_string())),
            ),
            // modifier too big
            (
                "1d20+10000000",
                Err(ParseRollError::InvalidFormat("1d20+10000000".to_string())),
            ),
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
        ];

        for (input, expected) in cases {
            let actual = RollSettings::from_str(input);
            match expected {
                Ok(expected) => {
                    assert!(actual.is_ok());
                    assert_eq!(expected, actual.unwrap());
                }
                Err(e) => {
                    assert!(actual.is_err());
                    assert_eq!(e, actual.unwrap_err());
                }
            }
        }
    }
}
