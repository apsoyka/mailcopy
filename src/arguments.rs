use std::path::PathBuf;

use clap::{Args, Parser};
use log::LevelFilter;

#[derive(Parser)]
#[command(author, version, about, long_about = None, arg_required_else_help = false)]
#[command(propagate_version = true)]
pub struct Arguments {
    #[command(flatten)]
    pub verbosity: Verbosity,

    #[command(flatten)]
    pub authentication: Authentication,

    #[arg(help = "The name of the host to connect with")]
    pub hostname: String,

    #[arg(help = "The port number to connect with")]
    pub port: u16,

    #[arg(help = "A path on the filesystem to write to")]
    pub output: PathBuf
}

#[derive(Args)]
#[group(multiple = false)]
pub struct Verbosity {
    #[arg(short = 'd', long = "debug", help = "Enable debugging output")]
    pub debug: bool,

    #[arg(short = 'v', long = "verbose", help = "Enable verbose output")]
    pub verbose: bool,

    #[arg(short = 'q', long = "quiet", help = "Suppress informational messages")]
    pub quiet: bool
}

#[derive(Args)]
#[group()]
pub struct Authentication {
    #[arg(short = 'u', long = "username", help = "The username to use for authentication")]
    pub username: Option<String>,

    #[arg(short = 'p', long = "password", help = "The password to use for authentication")]
    pub password: Option<String>,

    #[arg(short = 'i', long = "insecure", help = "Accept invalid TLS certificates")]
    pub insecure: bool
}

impl Verbosity {
    pub fn to_filter(&self) -> LevelFilter {
        if self.debug { LevelFilter::Trace }
        else if self.verbose { LevelFilter::Debug }
        else if self.quiet { LevelFilter::Warn }
        else { LevelFilter::Info }
    }
}
