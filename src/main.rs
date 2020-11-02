use std::{env, error::Error};

use clap::{Clap, crate_authors, crate_version};

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

async fn fetch_slack_custom_emojis(token: &str, workspace: &str) -> Result<(), Box <dyn Error>> {
    let url = format!("https://{}.slack.com/api/emoji.adminList", workspace);
    let client = reqwest::Client::new();
    let response: serde_json::Value = client.post(&url)
        .form(&[("token", token)])
        .send()
        .await?
        .json()
        .await?;
    println!("{:#?}", response);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opts = Opts::parse();

    let slack_token = get_slack_token();
    // println!("Output archive file: {}", opts.archive_file);
    fetch_slack_custom_emojis(&slack_token, &opts.slack_workspace).await?;
    return Ok(())
}
