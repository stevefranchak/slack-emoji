use futures::future::Either::{Left, Right};
use std::error::Error;

use crate::emoji::EmojiStreamParameters;
use actions::{download, upload};
use cli::{get_opts, SubCommandKind};

mod actions;
mod archive;
mod cli;
mod emoji;
mod slack;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opts = get_opts();
    let slack_client = opts.create_slack_client();
    let target_directory = &opts.target_directory;
    match opts.subcommand {
        SubCommandKind::Download { emoji_stream_opts } => Left(download(
            slack_client,
            target_directory,
            EmojiStreamParameters::from(&emoji_stream_opts),
        )),
        SubCommandKind::Upload => Right(upload(slack_client, target_directory)),
    }
    .await
}
