use std::error::Error;
use std::rc::Rc;

use clap::{crate_authors, crate_description, crate_version, Clap};
use env_logger::{Builder, Env};
use log::LevelFilter;

use actions::{download, upload};
use slack::SlackClient;

mod actions;
mod archive;
mod emoji;
mod slack;

#[derive(Clap)]
#[clap(version = crate_version!(), author = crate_authors!(), about = crate_description!())]
struct Opts {
    /// Slack workspace subdomain (e.g. if your Slack is at myorg.slack.com, enter "myorg")
    #[clap(name = "SLACK WORKSPACE")]
    workspace: String,
    /// Path to directory to either download emojis to or upload emojis from. The `download` subcommand will attempt
    /// to create a directory at the provided path if it does not exist, whereas the `upload` subcommand expects
    /// that the provided path is an existing directory containing a well-formed 'metadata.ndjson' and emoji files.
    #[clap(name = "TARGET DIRECTORY")]
    target_directory: String,
    /// Token for a user that has permissions for Slack's administrator-level emoji endpoints.
    /// The token can be manually acquired by inspecting the payload for a request, such as POST /api/emoji.adminList,
    /// via a browser's network dev tools when accessing a Slack workspace's customize/emoji page.
    /// The token generally starts with "xox".
    #[clap(
        name = "slack token",
        short = 't',
        long = "token",
        env = "SLACK_TOKEN",
        required = true
    )]
    token: String,
    /// Sets the log level based on occurrences. The default log level includes ERROR and WARN messages. One occurrence
    /// includes INFO messages, two occurrences include DEBUG messages, and three or more occurrences include TRACE
    /// messages. The log level can also be set via the environment variable "SLACK_EMOJI_LOG_LEVEL". This argument, if
    /// provided, takes precedence over the aforementioned environment variable.
    #[clap(short, parse(from_occurrences))]
    verbose: u8,
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    /// Downloads emojis from SLACK WORKSPACE to TARGET DIRECTORY
    Download,
    /// Uploads emojis to SLACK WORKSPACE from TARGET DIRECTORY
    Upload,
}

impl From<&Opts> for SlackClient {
    fn from(opts: &Opts) -> Self {
        Self::new(&opts.token, &opts.workspace)
    }
}

fn setup_logging(verbosity: u8) {
    let env = Env::default()
        .filter_or("SLACK_EMOJI_LOG_LEVEL", "warn")
        .write_style_or("SLACK_EMOJI_LOG_STYLE", "always"); // "never" disables color formatting

    let mut builder = Builder::new();
    builder.parse_env(env);
    if verbosity > 0 {
        builder.filter_level(match verbosity {
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        });
    }
    builder.init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opts = Opts::parse();
    setup_logging(opts.verbose);
    let slack_client = Rc::new(SlackClient::from(&opts));
    match opts.subcmd {
        SubCommand::Download => download(slack_client, &opts.target_directory).await,
        SubCommand::Upload => upload(slack_client, &opts.target_directory).await,
    }
}
