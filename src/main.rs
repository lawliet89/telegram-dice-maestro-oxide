mod cli;
mod dice;
mod dnd;
mod parser;
mod storage;

use std::str::FromStr;

use anyhow::anyhow;
use clap::Parser;
use teloxide::adaptors::{CacheMe, DefaultParseMode, Throttle};
use teloxide::prelude::*;
use teloxide::requests::RequesterExt;
use teloxide::types::{InputFile, ParseMode};
use teloxide::utils::command::BotCommands;

use dice::*;

#[derive(BotCommands, Clone, PartialEq)]
#[command(
    rename_rule = "snake_case",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Display help text")]
    Help,
    #[command(description = "Roll die.")]
    Roll(String),
    #[command(description = "Roll die, and send data output")]
    Data(String),
    #[command(description = "Roll with advantage")]
    Adv(String),
    #[command(description = "Roll with advantage")]
    Advantage(String),
    #[command(description = "Roll with advantage, and send data output")]
    AdvantageData(String),
    #[command(description = "Roll with disadvantage")]
    Dis(String),
    #[command(description = "Roll with disadvantage")]
    Disadvantage(String),
    #[command(description = "Roll with disadvantage, and send data output")]
    DisadvantageData(String),
}

fn get_token<S1, S2>(token: Option<S1>, file: Option<S2>) -> anyhow::Result<String>
where
    S1: std::string::ToString,
    S2: AsRef<std::path::Path>,
{
    if let Some(token) = token.as_ref() {
        return Ok(token.to_string());
    }
    if let Some(file) = file.as_ref() {
        return Ok(std::fs::read_to_string(file)?.trim().to_string());
    }
    Err(anyhow!("No API Key provided"))
}

type AdaptedBot = DefaultParseMode<Throttle<CacheMe<Bot>>>;

async fn answer(bot: AdaptedBot, msg: Message, cmd: Command) -> ResponseResult<()> {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::Roll(input) => {
            handle_roll(bot, msg, input.as_str(), &RollType::Straight, false).await?
        }
        Command::Data(input) => {
            handle_roll(bot, msg, input.as_str(), &RollType::Straight, true).await?
        }
        Command::Advantage(input) | Command::Adv(input) => {
            handle_roll(bot, msg, input.as_str(), &RollType::Advantage, false).await?
        }
        Command::AdvantageData(input) => {
            handle_roll(bot, msg, input.as_str(), &RollType::Advantage, true).await?
        }
        Command::Disadvantage(input) | Command::Dis(input) => {
            handle_roll(bot, msg, input.as_str(), &RollType::Disadvantage, false).await?
        }
        Command::DisadvantageData(input) => {
            handle_roll(bot, msg, input.as_str(), &RollType::Disadvantage, true).await?
        }
    };

    Ok(())
}

async fn handle_roll(
    bot: AdaptedBot,
    msg: Message,
    input: &str,
    roll_type: &RollType,
    send_json: bool,
) -> ResponseResult<()> {
    let silly_text =  "As a non-language non-model, I just spit out what was written in my code and I can never vary.";
    match input {
        "" => {
            bot.send_dice(msg.chat.id)
                .reply_to_message_id(msg.id)
                .await?;
        }
        "eye" | "eyes" | "👀" | "👁" | "👁‍🗨" => {
            bot.send_message(msg.chat.id, silly_text)
                .reply_to_message_id(msg.id)
                .await?;
        }
        input => {
            let settings = RollSettings::from_str(input);
            match settings {
                Ok(settings) => {
                    let results = RollResults::new(&settings, roll_type);
                    log::debug!("Dice roll: {:?}", results);
                    let roll_msg = bot
                        .send_message(msg.chat.id, results.to_string())
                        .reply_to_message_id(msg.id)
                        .await?;
                    if send_json {
                        match serde_json::to_string_pretty(&results) {
                            Ok(output_json) => {
                                bot.send_document(
                                    msg.chat.id,
                                    InputFile::memory(output_json.into_bytes())
                                        .file_name("roll.json"),
                                )
                                .reply_to_message_id(roll_msg.id)
                                .await?;
                            }
                            Err(e) => {
                                bot.send_message(
                        msg.chat.id,
                        format!("Could not convert results to JSON. This is a bug in the bot.\n\n<code>{}</code>", e),
                    )
                    .reply_to_message_id(msg.id)
                    .await?;
                            }
                        }
                        // bot.send_document(msg.chat.id, document)
                    }
                }
                Err(e) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("{} \n\nIn other words, it is likely you have made a mistake and I definitely cannot help you to fix it. Try again!\n\n💣 <code>{}</code> 💣", silly_text, e),
                    )
                    .reply_to_message_id(msg.id)
                    .await?;
                }
            }
        }
    }
    Ok(())
}

async fn run_bot(args: &cli::RunArgs) -> anyhow::Result<()> {
    log::info!("Reading token...");
    let token = get_token(args.bot_token_file.as_ref(), args.bot_token.as_ref())?;
    let bot = Bot::new(token)
        .cache_me()
        .throttle(Default::default())
        .parse_mode(ParseMode::Html);

    log::info!("Starting die rolling bot...");
    log::info!("Running as: {:?}", bot.get_me().await?);

    if args.set_my_commands {
        let commands = Command::bot_commands();
        log::info!("Setting bot commands: {:?}", commands);
        bot.set_my_commands(commands).await?;
    }

    Command::repl(bot, answer).await;
    Ok(())
}

async fn load_character_data(path: &str) -> anyhow::Result<()> {
    let (ok, err) = dnd::Character::load_from_pattern(path)?;
    for character in ok {
        log::info!("Loaded {:#?}", character);
    }
    for error in err {
        log::error!("{:#?}", error);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::formatted_timed_builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    let cli = cli::Cli::parse();
    log::debug!("Command line: {:?}", cli);

    match cli.command {
        None => {
            println!("{:?}", cli);
        }
        Some(cli::Command::Run(args)) => {
            run_bot(&args).await?;
        }
        Some(cli::Command::LoadCharacterData { path }) => load_character_data(&path).await?,
    }

    Ok(())
}
