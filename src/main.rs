use std::str::FromStr;

use anyhow::anyhow;
use clap::Parser;
use lazy_static::lazy_static;
use rand::distributions::{Distribution, Uniform};
use regex::Regex;
use teloxide::adaptors::{CacheMe, DefaultParseMode, Throttle};
use teloxide::prelude::*;
use teloxide::requests::RequesterExt;
use teloxide::types::ParseMode;
use teloxide::utils::command::{BotCommands, ParseError};
use thiserror::Error;

/// Telegram bot to roll a dice!
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to API Key
    #[arg(
        long,
        env,
        conflicts_with("api_key"),
        required_unless_present("api_key")
    )]
    api_key_file: Option<String>,

    /// API Key. **Highly recommended that this is not set this via command line.**
    #[arg(long, env, required_unless_present("api_key_file"))]
    api_key: Option<String>,
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "snake_case",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Display help text")]
    Help,
    #[command(description = "Roll a dice.")]
    Roll(String),
}

fn identity_parser(input: String) -> Result<String, ParseError> {
    Ok(input)
}

fn get_token(args: Args) -> anyhow::Result<String> {
    if let Some(key) = args.api_key {
        return Ok(key);
    }
    if let Some(file) = args.api_key_file {
        return Ok(std::fs::read_to_string(file)?.trim().to_string());
    }
    Err(anyhow!("No API Key provided"))
}

#[derive(Error, Debug)]
enum ParseRollError {
    #[error("Invalid roll format: {0}")]
    InvalidFormat(String),
    #[error("Number of dices and dice sides cannot be zero: {0}")]
    CannotBeZero(String),
}

impl From<ParseRollError> for ParseError {
    fn from(parse_err: ParseRollError) -> Self {
        use ParseRollError::*;

        match parse_err {
            e @ InvalidFormat(_) | e @ CannotBeZero(_) => Self::IncorrectFormat(Box::new(e)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RollSettings {
    pub number: u32,
    pub sides: u32,
    pub modifier: Option<i32>,
}

impl RollSettings {
    fn roll(&self) -> RollResults {
        RollResults::new(self)
    }
}

impl FromStr for RollSettings {
    type Err = ParseRollError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        lazy_static! {
            static ref RE: Regex =
                Regex::new(r"^([0-9]{1,4})(d|D)([0-9]{1,4})([+-][0-9]{1,4})?$").unwrap();
        }
        log::trace!("Cleaning raw input {}", input);
        let stripped: String = input.chars().filter(|c| !c.is_whitespace()).collect();
        log::info!("Parsing input {}", input);
        let captures = RE.captures(&stripped).ok_or_else(|| {
            log::warn!("Regex match failure for {}", input);
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

        Ok(Self {
            number,
            sides,
            modifier,
        })
    }
}

#[derive(Debug)]
struct RollResults<'a> {
    rolls: Vec<u32>,
    settings: &'a RollSettings,
}

impl<'a> RollResults<'a> {
    fn new(settings: &RollSettings) -> Self {
        let mut rng = rand::thread_rng();
        let die = Uniform::from(1..=settings.sides);

        let rolls = (1..=settings.number)
            .map(|_| die.sample(&mut rng))
            .collect();

        RollResults { settings, rolls }
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
                    format!(" - {}", number * -1)
                }
            }
        };

        writeln!(
            f,
            "Parameters: {} d {}{}",
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

        let total = self.rolls.iter().fold(0, |a, b| a + b);
        let mut total: i64 = total.into();
        if let Some(modifier) = self.settings.modifier {
            total = total + modifier as i64
        }
        write!(f, "Your final roll is: 🎲 <b>{}</b> 🎲", total)
    }
}

type AdaptedBot = DefaultParseMode<Throttle<CacheMe<Bot>>>;
async fn answer(bot: AdaptedBot, msg: Message, cmd: Command) -> ResponseResult<()> {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::Roll(input) => {
            if input == "" {
                bot.send_dice(msg.chat.id)
                    .reply_to_message_id(msg.id)
                    .await?;
            } else {
                let settings = RollSettings::from_str(&input);
                match settings {
                    Ok(settings) => {
                        let results = settings.roll();
                        log::debug!("Dice roll: {:?}", results);
                        bot.send_message(msg.chat.id, results.to_string())
                            .reply_to_message_id(msg.id)
                            .await?;
                    }
                    Err(e) => {
                        bot.send_message(
                            msg.chat.id,
                            format!("😢 I did not understand you: {}", e),
                        )
                        .reply_to_message_id(msg.id)
                        .await?;
                    }
                }
            }
        }
    };

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    let args = Args::parse();
    log::info!("Reading token...");
    let token = get_token(args)?;
    let bot = Bot::new(token)
        .cache_me()
        .throttle(Default::default())
        .parse_mode(ParseMode::Html);

    log::info!("Starting dicer roller bot...");

    Command::repl(bot, answer).await;
    Ok(())
}
