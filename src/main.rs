use std::{env, error::Error};
use std::rc::Rc;

use clap::{Clap, crate_authors, crate_version};
use futures::pin_mut;
use futures::stream::StreamExt;
use serde_json;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use emoji::EmojiPaginator;
use slack::SlackClient;

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
    /// Filepath of the output archive file (including filename)
    #[clap(name = "OUTPUT ARCHIVE FILE")]
    archive_file: String,
    /// Slack workspace subdomain (e.g. if your Slack is at myorg.slack.com, enter "myorg")
    #[clap(name = "SLACK WORKSPACE")]
    slack_workspace: String,
}

static SLACK_TOKEN_ENV_VAR_NAME: &str = "SLACK_TOKEN";

fn get_slack_token() -> String {
    match env::var(SLACK_TOKEN_ENV_VAR_NAME) {
        Ok(value) => value,
        Err(e) => panic!("Encountered error when trying to access env var {}: {}", SLACK_TOKEN_ENV_VAR_NAME, e)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opts = Opts::parse();
    let slack_client = Rc::new(SlackClient::new(get_slack_token(), &opts.slack_workspace.to_string()));

    let stream = EmojiPaginator::new(slack_client, 100).into_stream();
    pin_mut!(stream);

    let mut num_emojis: u16 = 0;
    while let Some(Ok(emoji)) = stream.next().await {
        println!("{}", serde_json::to_string(&emoji)?);
        num_emojis += 1;
    }
    println!("There are {} custom emojis", num_emojis);

    // Generate random directory
    let mut temp_dir_path = env::temp_dir();
    temp_dir_path.push("slack-emoji-exporter");
    fs::remove_dir_all(&temp_dir_path).await?;
    fs::create_dir(&temp_dir_path).await?;

    let mut metadata_filepath = temp_dir_path.clone();
    metadata_filepath.push("metadata.csv");
    let mut metadata_file = fs::File::create(&metadata_filepath).await?;
    metadata_file.write_all(b"hello, world!\n").await?;
    metadata_file.flush().await?;

    // fs::remove_dir_all(&temp_dir_path).await?;

    return Ok(())
}
