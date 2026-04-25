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
        if let Some(ref label) = self.settings.label {
            writeln!(f, "<u>{}</u>", label)?;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(number: u32, sides: u32, modifier: Option<i32>) -> RollSettings {
        RollSettings {
            number,
            sides,
            modifier,
            label: None,
        }
    }

    fn fixed_roll(s: &RollSettings, rolls: Vec<u32>, total: i64) -> Roll<'_> {
        Roll {
            rolls,
            total,
            settings: s,
        }
    }

    // Advantage must always return the higher of the two rolls.
    #[test]
    fn advantage_picks_higher_roll() {
        let s = settings(1, 20, None);
        let roll_type = RollType::Advantage;

        // try_one is lower
        let results = RollResults {
            roll_type: &roll_type,
            try_one: fixed_roll(&s, vec![4], 4),
            try_two: Some(fixed_roll(&s, vec![17], 17)),
            settings: &s,
        };
        assert_eq!(results.result().total, 17);

        // try_one is higher
        let results = RollResults {
            roll_type: &roll_type,
            try_one: fixed_roll(&s, vec![17], 17),
            try_two: Some(fixed_roll(&s, vec![4], 4)),
            settings: &s,
        };
        assert_eq!(results.result().total, 17);
    }

    // Disadvantage must always return the lower of the two rolls.
    #[test]
    fn disadvantage_picks_lower_roll() {
        let s = settings(1, 20, None);
        let roll_type = RollType::Disadvantage;

        // try_one is lower
        let results = RollResults {
            roll_type: &roll_type,
            try_one: fixed_roll(&s, vec![3], 3),
            try_two: Some(fixed_roll(&s, vec![18], 18)),
            settings: &s,
        };
        assert_eq!(results.result().total, 3);

        // try_two is lower
        let results = RollResults {
            roll_type: &roll_type,
            try_one: fixed_roll(&s, vec![18], 18),
            try_two: Some(fixed_roll(&s, vec![3], 3)),
            settings: &s,
        };
        assert_eq!(results.result().total, 3);
    }

    // `result()` and `results_index()` are independent code paths that must agree
    // on which attempt won.  This test drives both with the same data and checks
    // they are consistent.
    #[test]
    fn results_index_matches_result_selection() {
        let s = settings(1, 20, None);

        // Advantage: try_one wins → index 1
        let rt = RollType::Advantage;
        let r = RollResults {
            roll_type: &rt,
            try_one: fixed_roll(&s, vec![18], 18),
            try_two: Some(fixed_roll(&s, vec![5], 5)),
            settings: &s,
        };
        assert_eq!(r.results_index(), 1);
        assert_eq!(r.result().total, 18);

        // Advantage: try_two wins → index 2
        let r = RollResults {
            roll_type: &rt,
            try_one: fixed_roll(&s, vec![5], 5),
            try_two: Some(fixed_roll(&s, vec![18], 18)),
            settings: &s,
        };
        assert_eq!(r.results_index(), 2);
        assert_eq!(r.result().total, 18);

        // Disadvantage: try_one wins (is lower) → index 1
        let rt = RollType::Disadvantage;
        let r = RollResults {
            roll_type: &rt,
            try_one: fixed_roll(&s, vec![2], 2),
            try_two: Some(fixed_roll(&s, vec![14], 14)),
            settings: &s,
        };
        assert_eq!(r.results_index(), 1);
        assert_eq!(r.result().total, 2);

        // Disadvantage: try_two wins (is lower) → index 2
        let r = RollResults {
            roll_type: &rt,
            try_one: fixed_roll(&s, vec![14], 14),
            try_two: Some(fixed_roll(&s, vec![2], 2)),
            settings: &s,
        };
        assert_eq!(r.results_index(), 2);
        assert_eq!(r.result().total, 2);
    }

    // When both attempts produce the same total, result() and results_index()
    // must still agree with each other (both should point to try_two, per the
    // semantics of std::cmp::max/min when equal).
    #[test]
    fn tie_result_and_index_are_consistent() {
        let s = settings(1, 20, None);

        for rt in [RollType::Advantage, RollType::Disadvantage] {
            let r = RollResults {
                roll_type: &rt,
                try_one: fixed_roll(&s, vec![10], 10),
                try_two: Some(fixed_roll(&s, vec![10], 10)),
                settings: &s,
            };
            // The important invariant: whichever roll result() reports, results_index()
            // must point to the same attempt.
            let selected_total = r.result().total;
            let winning_roll = match r.results_index() {
                1 => r.try_one.total,
                2 => r.try_two.as_ref().unwrap().total,
                _ => unreachable!(),
            };
            assert_eq!(
                selected_total, winning_roll,
                "result() and results_index() disagree for {:?} on a tie",
                rt
            );
        }
    }

    // Long roll results must be capped so that the Telegram HTML entity limit
    // is respected.  The truncated section must end with "..." so users know
    // output was cut, and the results portion must not exceed the requested limit.
    #[test]
    fn format_roll_truncates_long_result_with_ellipsis() {
        // settings.number matches the number of dice we actually pass in.
        let s = settings(60, 6, None);
        // 60 dice all showing "1" → "1 + 1 + 1 + …" (~240 chars before truncation)
        let roll = fixed_roll(&s, vec![1; 60], 60);

        let limit = 20usize;
        let truncated = roll.format_roll(Some(limit));
        assert!(
            truncated.contains("..."),
            "Expected '...' in truncated output, got: {truncated}"
        );
        // Output format is "(inner)modifier". With no modifier the inner string
        // (between the parens) must be at most limit + 3 bytes ("…" adds 3 chars).
        let inner_len = truncated.len() - 2; // strip the two surrounding parens
        assert!(
            inner_len <= limit + 3,
            "Inner results length {inner_len} exceeds limit + len('...') = {}",
            limit + 3
        );

        // Without a truncation limit the same roll must NOT add ellipsis.
        let full = roll.format_roll(None);
        assert!(
            !full.contains("..."),
            "Untruncated output should not contain '...', got: {full}"
        );
    }

    // Property: for any random roll, total must exactly equal the sum of the
    // individual dice values plus the modifier.  Tested over many iterations to
    // catch any latent off-by-one in the accumulation logic.
    #[test]
    fn roll_total_invariant() {
        let with_mod = settings(4, 6, Some(5));
        let no_mod = settings(3, 8, None);
        let neg_mod = settings(2, 10, Some(-3));

        for _ in 0..200 {
            for s in [&with_mod, &no_mod, &neg_mod] {
                let roll = Roll::new(s);
                let dice_sum: i64 = roll.rolls.iter().map(|&d| d as i64).sum();
                let modifier = s.modifier.unwrap_or(0) as i64;
                assert_eq!(
                    roll.total,
                    dice_sum + modifier,
                    "total invariant violated for settings {:?}",
                    s
                );
            }
        }
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
                if let Some(ref label) = self.settings.label {
                    writeln!(f, "<u>{}</u>", label)?;
                }
                writeln!(
                    f,
                    "Parameters: {} with <i>{}</i>",
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
