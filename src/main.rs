use std::{env, error::Error};
use std::rc::Rc;

use clap::{Clap, crate_authors, crate_version};
use futures::pin_mut;
use futures::stream::StreamExt;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use archive::EmojiFile;
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
    let slack_client = Rc::new(
        SlackClient::new(get_slack_token(), &opts.slack_workspace.to_string())
    );

    let stream = EmojiPaginator::new(slack_client.clone(), 100).into_stream();
    pin_mut!(stream);

    // Generate temporary directory that the archive will be created from
    // TODO: refactor most of this as impl of an EmojiArchive struct
    let mut temp_dir_path = env::temp_dir();
    temp_dir_path.push("slack-emoji-exporter");
    fs::remove_dir_all(&temp_dir_path).await?;
    fs::create_dir(&temp_dir_path).await?;
    println!("Temp directory located at {}", &temp_dir_path.to_str().unwrap());

    let mut metadata_filepath = temp_dir_path.clone();
    metadata_filepath.push("metadata.ndjson");
    let mut metadata_file = fs::File::create(&metadata_filepath).await?;

    // TODO: separate consuming of stream results into a separate task (or task pool); measure perf
    while let Some(Ok(emoji)) = stream.next().await {
        let emoji_file = EmojiFile::new(emoji);
        emoji_file.download_to_file(slack_client.clone(), &temp_dir_path).await?;

        let mut emoji_bytes = serde_json::to_vec(&emoji_file)?;
        emoji_bytes.extend_from_slice(b"\n");
        metadata_file.write_all(&emoji_bytes).await?;
    }
    metadata_file.flush().await?;

    // TODO: create archive at archive_file, cleanup temp directory
    // fs::remove_dir_all(&temp_dir_path).await?;

    Ok(())
}
