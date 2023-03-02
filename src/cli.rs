use crate::emoji::{EmojiStreamParameters, DEFAULT_NUM_EMOJIS_PER_PAGE, DEFAULT_STARTING_PAGE};
use crate::slack::SlackClient;
use clap::{ArgAction, Args, Parser, Subcommand};
use env_logger::Env;
use log::LevelFilter;
use std::rc::Rc;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Opts {
    /// Slack workspace subdomain (e.g. if your Slack is at myorg.slack.com, enter "myorg")
    #[clap(name = "SLACK WORKSPACE")]
    workspace: String,
    /// Path to directory to either download emojis to or upload emojis from. The `download` subcommand will attempt
    /// to create a directory at the provided path if one does not exist. The `upload` subcommand expects
    /// that the provided path is an existing directory containing a well-formed 'metadata.ndjson' and emoji files.
    #[clap(name = "TARGET DIRECTORY")]
    pub target_directory: String,
    /// Token for a user that has permissions for Slack's administrator-level emoji endpoints.
    /// The token can be manually acquired by inspecting the payload for a request, such as POST /api/emoji.adminList,
    /// via a browser's network dev tools when accessing a Slack workspace's customize/emoji page.
    /// The token generally starts with "xox".
    ///
    /// It is STRONGLY advised to provide this argument via the environment variable SLACK_TOKEN.
    #[clap(
        name = "slack token",
        short = 't',
        long = "token",
        env = "SLACK_TOKEN",
        required = true
    )]
    token: String,
    /// It is STRONGLY advised to provide this argument via the environment variable SLACK_SESSION_COOKIE.
    #[clap(
        name = "slack session cookie",
        short = 'd',
        long = "session_cookie",
        env = "SLACK_SESSION_COOKIE",
        required = true
    )]
    session_cookie: String,
    /// Sets the log level based on occurrences. The default log level includes ERROR and WARN messages. One occurrence
    /// includes INFO messages, two occurrences include DEBUG messages, and three or more occurrences include TRACE
    /// messages. The log level can also be set via the environment variable SLACK_EMOJI_LOG_LEVEL. This argument, if
    /// provided, takes precedence over the aforementioned environment variable.
    #[clap(name = "verbose", short, action(ArgAction::Count))]
    verbosity: u8,
    // #[clap(long, required = false)]
    // filter_by_uploader: Option<String>,
    #[clap(subcommand)]
    pub subcommand: SubCommandKind,
}

#[derive(Args)]
pub struct EmojiStreamOpts {
    #[clap(long, required = false, default_value_t = DEFAULT_STARTING_PAGE)]
    starting_page_number: u16,
    #[clap(long, required = false, default_value_t = DEFAULT_NUM_EMOJIS_PER_PAGE)]
    num_emojis_per_page: u8,
    #[clap(long)]
    limit_num_pages: Option<u16>,
}

#[derive(Subcommand)]
pub enum SubCommandKind {
    /// Downloads emojis from SLACK WORKSPACE to TARGET DIRECTORY
    Download {
        #[clap(flatten)]
        emoji_stream_opts: EmojiStreamOpts,
    },
    /// Uploads emojis to SLACK WORKSPACE from TARGET DIRECTORY
    Upload,
}

impl From<&Opts> for SlackClient {
    fn from(opts: &Opts) -> Self {
        Self::new(&opts.token, &opts.session_cookie, &opts.workspace)
    }
}

impl From<&EmojiStreamOpts> for EmojiStreamParameters {
    fn from(opts: &EmojiStreamOpts) -> Self {
        Self::new(
            opts.starting_page_number,
            opts.num_emojis_per_page,
            opts.limit_num_pages,
        )
    }
}

impl Opts {
    fn setup_logging(self) -> Self {
        let verbosity = self.verbosity;
        let env = Env::default()
            .filter_or("SLACK_EMOJI_LOG_LEVEL", "warn")
            .write_style_or("SLACK_EMOJI_LOG_STYLE", "always"); // "never" disables color formatting

        let mut builder = env_logger::Builder::new();
        builder.parse_env(env);
        if verbosity > 0 {
            builder.filter_level(match verbosity {
                1 => LevelFilter::Info,
                2 => LevelFilter::Debug,
                _ => LevelFilter::Trace,
            });
        }
        builder.init();
        self
    }

    pub fn create_slack_client(&self) -> Rc<SlackClient> {
        Rc::new(SlackClient::from(self))
    }
}

pub fn get_opts() -> Opts {
    Opts::parse().setup_logging()
}
