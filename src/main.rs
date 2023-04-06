mod dice;

use std::str::FromStr;

use anyhow::anyhow;
use clap::Parser;
use teloxide::adaptors::{CacheMe, DefaultParseMode, Throttle};
use teloxide::prelude::*;
use teloxide::requests::RequesterExt;
use teloxide::types::{InputFile, ParseMode};
use teloxide::utils::command::BotCommands;

use dice::*;

/// Telegram bot to roll a dice!
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to file containing Telegram Bot Token
    #[arg(
        long,
        env,
        conflicts_with("bot_token"),
        required_unless_present("bot_token")
    )]
    bot_token_file: Option<String>,

    /// Bot token. **Highly recommended that this is not set via command line, because it will show up in running processes.**
    #[arg(long, env, required_unless_present("bot_token_file"))]
    bot_token: Option<String>,
}

#[derive(BotCommands, Clone, PartialEq)]
#[command(
    rename_rule = "snake_case",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Display help text")]
    Help,
    #[command(description = "Roll a dice.")]
    Roll(String),
    #[command(description = "Roll a dice, and send data output")]
    RollWithData(String),
}

fn get_token(args: Args) -> anyhow::Result<String> {
    if let Some(key) = args.bot_token {
        return Ok(key);
    }
    if let Some(file) = args.bot_token_file {
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
        Command::Roll(input) => handle_roll(bot, msg, input.as_str(), false).await?,
        Command::RollWithData(input) => handle_roll(bot, msg, input.as_str(), true).await?,
    };

    Ok(())
}

async fn handle_roll(
    bot: AdaptedBot,
    msg: Message,
    input: &str,
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
                    let results = settings.roll();
                    log::debug!("Dice roll: {:?}", results);
                    let roll_msg = bot
                        .send_message(msg.chat.id, results.to_string())
                        .reply_to_message_id(msg.id)
                        .await?;
                    if send_json {
                        match serde_json::to_string_pretty(&results) {
                            Ok(output_json) => {
                                // https://github.com/teloxide/teloxide/discussions/869
                                #[cfg(not(feature = "tempfile-send"))]
                                {
                                    bot.send_document(
                                        msg.chat.id,
                                        InputFile::memory(output_json.into_bytes())
                                            .file_name("roll.json"),
                                    )
                                    .reply_to_message_id(roll_msg.id)
                                    .await?;
                                }
                                #[cfg(feature = "tempfile-send")]
                                {
                                    use std::io::Write;
                                    use tempfile::NamedTempFile;

                                    let mut temp_json = NamedTempFile::new()?;
                                    temp_json.write_all(output_json.as_bytes())?;
                                    temp_json.flush()?;
                                    bot.send_document(
                                        msg.chat.id,
                                        InputFile::file(temp_json.path()).file_name("roll.json"),
                                    )
                                    .reply_to_message_id(roll_msg.id)
                                    .await?;
                                }
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::formatted_timed_builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    let args = Args::parse();
    log::info!("Reading token...");
    let token = get_token(args)?;
    let bot = Bot::new(token)
        .cache_me()
        .throttle(Default::default())
        .parse_mode(ParseMode::Html);

    log::info!("Starting dicer roller bot...");
    log::info!("Running as: {:?}", bot.get_me().await?);

    Command::repl(bot, answer).await;
    Ok(())
}
