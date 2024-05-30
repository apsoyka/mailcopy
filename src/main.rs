mod arguments;

mod mail;

use std::{env, fs::File, net::TcpStream, path::{Path, PathBuf}};

use arguments::{Arguments, Verbosity};
use chrono::TimeDelta;
use imap::Client;
use mail::fetch_messages;
use clap::Parser;
use indicatif::{HumanBytes, MultiProgress};
use indicatif_log_bridge::LogWrapper;
use log::{debug, info};
use native_tls::{TlsConnector, TlsStream};
use tar::Builder;
use zstd::stream::AutoFinishEncoder;

type UnitResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
type MultiProgressResult = Result<MultiProgress, Box<dyn std::error::Error + Send + Sync>>;
type ClientResult = Result<Client<TlsStream<TcpStream>>, Box<dyn std::error::Error + Send + Sync>>;
type TupleResult = Result<(String, String), Box<dyn std::error::Error + Send + Sync>>;
type BuilderResult<W> = Result<Builder<W>, Box<dyn std::error::Error + Send + Sync>>;

const IMAP_USERNAME: &str = "IMAP_USERNAME";
const IMAP_PASSWORD: &str = "IMAP_PASSWORD";

const COMPRESSION_LEVEL: i32 = 3;

trait Format {
    fn format(&self) -> String;
}

impl Format for TimeDelta {
    fn format(&self) -> String {
        let seconds = self.num_seconds() % 60;
        let minutes = self.num_seconds() / 60 % 60;
        let hours = self.num_seconds() / 60 / 60;

        format!("{:0>2}:{:0>2}:{:0>2}", hours, minutes, seconds)
    }
}

fn setup_logging(verbosity: &Verbosity) -> MultiProgressResult {
    let filter = verbosity.to_filter();

    let logger = env_logger::builder()
        .filter_level(filter)
        .format_level(true)
        .format_target(false)
        .format_module_path(false)
        .format_timestamp_secs()
        .parse_default_env()
        .build();

    let multi_progress = MultiProgress::new();

    LogWrapper::new(multi_progress.clone(), logger).try_init()?;

    Ok(multi_progress)
}

fn init_connection(hostname: &str, port: u16, insecure: bool, starttls: bool, debug: bool) -> ClientResult {
    let tls = TlsConnector::builder()
        .danger_accept_invalid_certs(insecure)
        .build()?;

    let address = (hostname, port);

    let mut client = if starttls {
        imap::connect_starttls(address, hostname, &tls)?
    }
    else {
        imap::connect(address, hostname, &tls)?
    };

    client.debug = debug;

    Ok(client)
}

fn get_credentials(username: Option<String>, password: Option<String>) -> TupleResult {
    match dotenv::dotenv() {
        Ok(path) => debug!("Loaded credentials from {path:?}"),
        Err(error) => debug!("{error}")
    };

    let credentials = (
        username.or(env::var(IMAP_USERNAME).ok()).ok_or("Must provide a username")?,
        password.or(env::var(IMAP_PASSWORD).ok()).ok_or("Must provide a password")?
    );

    Ok(credentials)
}

fn init_tar(output: &PathBuf) -> BuilderResult<AutoFinishEncoder<File>> {
    let path = Path::new(output);
    let file = File::options()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;

    let encoder = zstd::Encoder::new(file, COMPRESSION_LEVEL)?.auto_finish();
    let builder = tar::Builder::new(encoder);

    Ok(builder)
}

fn main() -> UnitResult {
    let arguments = Arguments::parse();

    let multi_progress = setup_logging(&arguments.verbosity)?;

    let client = init_connection(
        &arguments.hostname,
        arguments.port,
        arguments.authentication.insecure,
        arguments.authentication.starttls,
        arguments.verbosity.debug
    )?;

    let (username, password) = get_credentials(arguments.authentication.username, arguments.authentication.password)?;

    let mut session = client.login(username, password).map_err(|error| error.0)?;

    let mut builder = init_tar(&arguments.output)?;

    let (total, elapsed) = fetch_messages(&mut session, multi_progress, &mut builder)?;

    info!("Copy completed in {}", elapsed.format());
    info!("Total copy size is {}", HumanBytes(total));

    builder.finish()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use crate::Format;

    #[test]
    fn can_format_time_delta() {
        let elapsed =  TimeDelta::seconds(14249).format();

        assert_eq!("03:57:29", elapsed);
    }
}
