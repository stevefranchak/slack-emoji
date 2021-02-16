use std::error::Error;
use std::rc::Rc;

use clap::{crate_authors, crate_description, crate_version, Clap};

use actions::{export, import};
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
    /// Path to directory to either download emojis to or upload emojis from. The `export` subcommand will attempt
    /// to create a directory at the provided path if it does not exist, whereas the `import` subcommand expects
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
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    /// Downloads emojis from SLACK WORKSPACE to TARGET DIRECTORY
    Export,
    /// Uploads emojis to SLACK WORKSPACE from TARGET DIRECTORY
    Import,
}

impl From<&Opts> for SlackClient {
    fn from(opts: &Opts) -> Self {
        Self::new(&opts.token, &opts.workspace)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opts = Opts::parse();
    let slack_client = Rc::new(SlackClient::from(&opts));

    match opts.subcmd {
        SubCommand::Export => export(slack_client, &opts.target_directory).await,
        SubCommand::Import => import(slack_client, &opts.target_directory).await,
    }
}
