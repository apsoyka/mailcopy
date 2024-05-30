mod arguments;

mod mail;

use std::{env, fs::File, path::Path, process::exit};

use arguments::{Arguments, Verbosity};
use chrono::{Local, TimeDelta};
use mail::{WriteTask, write_messages};
use clap::Parser;
use indicatif::{HumanBytes, MultiProgress, ProgressBar, ProgressStyle};
use indicatif_log_bridge::LogWrapper;
use log::{debug, error, info, warn};
use native_tls::TlsConnector;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

type UnitResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
type MultiProgressResult = Result<MultiProgress, Box<dyn std::error::Error + Send + Sync>>;

const IMAP_USERNAME: &str = "IMAP_USERNAME";
const IMAP_PASSWORD: &str = "IMAP_PASSWORD";

const PROGRESS_STYLE: &str = "[{elapsed_precise}] {wide_bar:.cyan/blue} {pos}/{len} {msg}";

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

fn main() -> UnitResult {
    let arguments = Arguments::parse();
    let multi_progress = setup_logging(&arguments.verbosity)?;
    let style = ProgressStyle::with_template(PROGRESS_STYLE)?.progress_chars("#>-");

    if dotenv::dotenv().ok() == None { debug!("Dotfile is invalid or missing"); }

    let tls = TlsConnector::builder()
        .danger_accept_invalid_certs(arguments.authentication.insecure)
        .build()?;

    let address = (arguments.hostname.as_str(), arguments.port);

    let mut client = if arguments.authentication.starttls {
        imap::connect_starttls(address, &arguments.hostname, &tls)?
    }
    else {
        imap::connect(address, &arguments.hostname, &tls)?
    };

    client.debug = arguments.verbosity.debug;

    let username = arguments.authentication.username.or(env::var(IMAP_USERNAME).ok());
    let password = arguments.authentication.password.or(env::var(IMAP_PASSWORD).ok());

    if username.is_none() || password.is_none() {
        error!("Must provide a username and password");

        exit(1);
    }

    let mut session = client.login(username.unwrap(), password.unwrap()).map_err(|error| error.0)?;

    let path = Path::new(&arguments.output);
    let file = File::options()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;

    let mut writer = ZipWriter::new(file);

    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Zstd)
        .compression_level(Some(3))
        .unix_permissions(0o755);

    let messages = session.list(Some(""), Some("*"))?;
    let count = messages.len() as u64;
    let progress = multi_progress.add(ProgressBar::new(count));
    let mut total: u64 = 0;

    progress.set_style(style.clone());

    let start = Local::now();

    for name in &messages {
        let index = progress.position() + 1;
        let name = name.name();

        progress.set_message(format!("{name} [{}]", HumanBytes(total)));

        session.examine(name)?;

        match session.fetch("1:*", "RFC822") {
            Ok(messages) => {
                let task = WriteTask::new(&messages, name, &multi_progress, &style, &mut writer, options);
                let size = write_messages(task)?;

                total += size;

                info!("{index}/{count} -> {name} [{}]", HumanBytes(size));
            },
            Err(error) => warn!("{index}/{count} -> Skipping {name}: {error}")
        }

        progress.inc(1);
    }

    let end = Local::now();
    let elapsed = (end - start).format();

    info!("Copy completed in {elapsed}");
    info!("Total copy size is {}", HumanBytes(total));

    progress.finish_and_clear();
    multi_progress.remove(&progress);
    writer.finish()?;

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
