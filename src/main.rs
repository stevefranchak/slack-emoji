use std::rc::Rc;
use std::{env, error::Error};

use clap::{crate_authors, crate_version, Clap};
use futures::pin_mut;
use futures::stream::StreamExt;

use archive::{EmojiDirectory, EmojiFile};
use emoji::EmojiPaginator;
use slack::SlackClient;

mod archive;
mod emoji;
mod slack;

/// Exports all custom emojis from a Slack workspace.
///
/// Required env var:
///
///   * SLACK_TOKEN - Token for a user that has permissions for Slack's POST /api/emoji.adminList endpoint.
///   The token can be manually acquired by inspecting the above endpoint's request payload via a browser's
///   network dev tools when accessing a Slack workspace's customize/emoji page. The token generally starts with "xox".
#[derive(Clap)]
#[clap(version = crate_version!(), author = crate_authors!())]
struct Opts {
    /// Slack workspace subdomain (e.g. if your Slack is at myorg.slack.com, enter "myorg")
    #[clap(name = "SLACK WORKSPACE")]
    slack_workspace: String,
    /// Filepath of the output archive file (including filename)
    #[clap(name = "TARGET DIRECTORY")]
    target_directory: String,
}

static SLACK_TOKEN_ENV_VAR_NAME: &str = "SLACK_TOKEN";

fn get_slack_token() -> String {
    match env::var(SLACK_TOKEN_ENV_VAR_NAME) {
        Ok(value) => value,
        Err(e) => panic!(
            "Encountered error when trying to access env var {}: {}",
            SLACK_TOKEN_ENV_VAR_NAME, e
        ),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opts = Opts::parse();
    let slack_client = Rc::new(SlackClient::new(
        get_slack_token(),
        &opts.slack_workspace.to_string(),
    ));

    let stream = EmojiPaginator::new(slack_client.clone(), 100).into_stream();
    pin_mut!(stream);

    let mut emoji_directory = EmojiDirectory::new(&opts.target_directory);
    emoji_directory.ensure_exists().await;

    // TODO: separate consuming of stream results into a separate task (or task pool); measure perf
    while let Some(Ok(emoji)) = stream.next().await {
        EmojiFile::new(emoji)
            .download_to_directory(slack_client.clone(), &mut emoji_directory)
            .await?;

    }

    Ok(())
}
