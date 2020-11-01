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
    #[clap(name = "OUTPUT ARCHIVE FILE")]
    archive_file: String
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

    let slack_token = get_slack_token();
    println!("Output archive file: {}", opts.archive_file);
    println!("Slack token: {}", slack_token);
    println!("Temp dir: {:?}", env::temp_dir());
    return Ok(())
}
