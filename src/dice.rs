use std::cmp::{max, min, Ordering};
use std::str::FromStr;

use rand::distributions::{Distribution, Uniform};
use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct RollSettings {
    pub number: u32,
    pub sides: u32,
    pub modifier: Option<i32>,
    pub label: Option<String>,
}

impl RollSettings {
    pub fn format_modifier(&self) -> String {
        match self.modifier {
            None => "".to_string(),
            Some(number) => {
                if number > 0 {
                    format!(" + {}", number)
                } else {
                    format!(" - {}", -number)
                }
            }
        }
    }

    pub fn format_parameters(&self) -> String {
        format!("{}d{}{}", self.number, self.sides, self.format_modifier())
    }
}

impl std::fmt::Display for RollSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_parameters())
    }
}

impl FromStr for RollSettings {
    type Err = crate::parser::ParseRollError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        crate::parser::parse_roll(input)
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Roll<'a> {
    pub rolls: Vec<u32>,
    pub total: i64,
    pub settings: &'a RollSettings,
}

impl<'a> Roll<'a> {
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

        Roll {
            settings,
            rolls,
            total,
        }
    }

    fn format_results(&self) -> String {
        self.rolls
            .iter()
            .map(ToString::to_string)
            .reduce(|a, b| format!("{} + {}", a, b))
            .expect("to not be empty")
    }

    fn format_roll(&self, truncate: Option<usize>) -> String {
        let mut results = self.format_results();
        if let Some(truncate) = truncate {
            if results.len() > truncate {
                results.truncate(truncate);
                results.push_str("...");
            }
        }

        format!("({}){}", results, self.settings.format_modifier())
    }
}

impl<'a> std::fmt::Display for Roll<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Parameters: {}", self.settings)?;

        // https://stackoverflow.com/questions/68768069/telegram-error-badrequest-entities-too-long-error-when-trying-to-send-long-ma
        // tldr; limit is 9500
        writeln!(f, "Roll: {}", self.format_roll(Some(4000)))?;

        write!(f, "Your final roll is: 🎲 <b>{}</b> 🎲", self.total)
    }
}

impl<'a> Ord for Roll<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.total.cmp(&other.total)
    }
}

impl<'a> PartialOrd for Roll<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RollType {
    Straight,
    Advantage,
    Disadvantage,
}

impl std::fmt::Display for RollType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RollType::Straight => write!(f, "Straight"),
            RollType::Advantage => write!(f, "Advantage"),
            RollType::Disadvantage => write!(f, "Disadvantage"),
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct RollResults<'a> {
    pub roll_type: &'a RollType,
    pub try_one: Roll<'a>,
    pub try_two: Option<Roll<'a>>,
    pub settings: &'a RollSettings,
}

impl<'a> RollResults<'a> {
    pub fn new(settings: &'a RollSettings, roll_type: &'a RollType) -> Self {
        let try_one = Roll::new(settings);
        let try_two = match roll_type {
            RollType::Straight => None,
            RollType::Advantage | RollType::Disadvantage => Some(Roll::new(settings)),
        };

        RollResults {
            roll_type,
            try_one,
            try_two,
            settings,
        }
    }

    pub fn result(&self) -> &Roll<'a> {
        match self.roll_type {
            RollType::Straight => &self.try_one,
            RollType::Advantage => max(&self.try_one, self.try_two.as_ref().expect("to be some")),
            RollType::Disadvantage => {
                min(&self.try_one, self.try_two.as_ref().expect("to be some"))
            }
        }
    }

    /// 1 or 2
    pub fn results_index(&self) -> usize {
        match self.roll_type {
            RollType::Straight => 1,
            RollType::Advantage => {
                if &self.try_one > self.try_two.as_ref().expect("to be some") {
                    1
                } else {
                    2
                }
            }
            RollType::Disadvantage => {
                if &self.try_one < self.try_two.as_ref().expect("to be some") {
                    1
                } else {
                    2
                }
            }
        }
    }
}

impl<'a> std::fmt::Display for RollResults<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.roll_type {
            RollType::Straight => self.try_one.fmt(f),
            RollType::Advantage | RollType::Disadvantage => {
                let results_index = self.results_index();
                writeln!(
                    f,
                    "Parameters: {} with <u>{}</u>",
                    self.settings, self.roll_type
                )?;
                let attempt_one = format!("Attempt one: {}", self.try_one.format_roll(Some(2000)));
                if results_index == 2 {
                    writeln!(f, "<s>{}</s>", attempt_one)?;
                } else {
                    writeln!(f, "{}", attempt_one)?;
                }
                let attempt_two = format!(
                    "Attempt two: {}",
                    self.try_two
                        .as_ref()
                        .expect("to be some")
                        .format_roll(Some(2000))
                );
                if results_index == 1 {
                    writeln!(f, "<s>{}</s>", attempt_two)?;
                } else {
                    writeln!(f, "{}", attempt_two)?;
                }
                write!(
                    f,
                    "Your final roll is: 🎲 <b>{}</b> 🎲",
                    self.result().total
                )
            }
        }
    }
}
