use anyhow::anyhow;
use clap::Parser;
use lazy_static::lazy_static;
use rand::distributions::{Distribution, Uniform};
use regex::Regex;
use teloxide::{
    prelude::*,
    utils::command::{BotCommands, ParseError},
};
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

#[derive(BotCommands, Clone, PartialEq)]
#[command(
    rename_rule = "snake_case",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Display help text")]
    Help,
    #[command(description = "Roll a dice.", parse_with = crate::parse_roll)]
    Roll(u32, u32, Option<i32>),
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

#[derive(Debug)]
struct RollResults {
    number: u32,
    sides: u32,
    rolls: Vec<u32>,
    modifier: Option<i32>,
}

impl RollResults {
    fn new(number: u32, sides: u32, modifier: Option<i32>) -> Self {
        let mut rng = rand::thread_rng();
        let die = Uniform::from(1..=sides);

        let results = (1..=number).map(|_| die.sample(&mut rng)).collect();

        RollResults {
            number,
            sides,
            rolls: results,
            modifier,
        }
    }
}

impl std::fmt::Display for RollResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let modifier_text = match self.modifier {
            None => "".to_string(),
            Some(number) => {
                if number > 0 {
                    format!("+{}", number)
                } else {
                    format!("{}", number)
                }
            }
        };

        writeln!(
            f,
            "Parameters: {}d{}{}",
            self.number, self.sides, modifier_text
        )?;

        let mut results = self
            .rolls
            .iter()
            .map(ToString::to_string)
            .reduce(|a, b| format!("{}+{}", a, b)).expect("to not be empty");

        if results.len() > 4000 {
            results.truncate(4000);
            results.push_str("...");
        }

        // https://stackoverflow.com/questions/68768069/telegram-error-badrequest-entities-too-long-error-when-trying-to-send-long-ma
        // tldr; limit is 9500
        writeln!(f, "Roll: ({}){}", results, modifier_text)?;

        let total = self.rolls.iter().fold(0, |a, b| a + b);
        let mut total: i64 = total.into();
        if let Some(modifier) = self.modifier {
            total = total + modifier as i64
        }
        write!(f, "Your final roll is: 🎲 {} 🎲", total)
    }
}

fn roll(number: u32, sides: u32, modifier: Option<i32>) -> RollResults {
    RollResults::new(number, sides, modifier)
}

fn parse_roll(input: String) -> Result<(u32, u32, Option<i32>), ParseRollError> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^([0-9]{1,4})(d|D)([0-9]{1,4})([+-][0-9]{1,4})?$").unwrap();
    }
    log::trace!("Cleaning raw input {}", input);
    let stripped: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    log::info!("Parsing input {}", input);
    let captures = RE
        .captures(&stripped)
        .ok_or_else(|| {
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

    Ok((number, sides, modifier))
}

async fn answer(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::Roll(number, sides, modifier) => {
            let results = roll(number, sides, modifier);
            log::debug!("Dice roll: {:?}", results);
            bot.send_message(msg.chat.id, results.to_string()).await?;
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
    let bot = Bot::new(token);

    log::info!("Starting dicer roller bot...");

    Command::repl(bot, answer).await;
    Ok(())
}
