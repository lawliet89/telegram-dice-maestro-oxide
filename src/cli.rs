use clap::{Parser, Subcommand};

/// Telegram bot to roll die!
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, args_conflicts_with_subcommands = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    // https://github.com/clap-rs/clap/issues/3857#issuecomment-1239419407
    #[clap(flatten)]
    run: RunArgs,
}

/// Actions
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run bot
    Run(RunArgs),

    /// Load Character Data and validate it. Echoes the output back.
    LoadCharacterData {
        /// Path to a single file or a directory containing character data
        path: String,
    },
}

#[derive(Parser, Debug, Default)]
pub struct RunArgs {
    /// Path to file containing Telegram Bot Token
    #[arg(
        long,
        env,
        conflicts_with("bot_token"),
        required_unless_present("bot_token")
    )]
    pub bot_token_file: Option<String>,

    /// Bot token. **Highly recommended that this is not set via command line, because it will show up in running processes.**
    #[arg(long, env, required_unless_present("bot_token_file"))]
    pub bot_token: Option<String>,

    /// Set bot commands on startup
    #[arg(long, env)]
    pub set_my_commands: bool,
}
