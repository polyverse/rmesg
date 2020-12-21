/// rmesg - a rust-based dmesg implementation.
/// This CLI builds on top of the eponymous crate and provides a command-line utility.
///
use clap::{App, Arg};
#[cfg(feature = "async")]
use futures_util::stream::TryStreamExt;
use rmesg::error::RMesgError;
use rmesg::klogctl::{klog, klog_timestamps_enabled, KLogEntries, SUGGESTED_POLL_INTERVAL};
use std::error::Error;

#[derive(Debug)]
struct Options {
    follow: bool,
    clear: bool,
}

#[cfg_attr(feature = "async", tokio::main)]
#[cfg(feature = "async")]
async fn main() -> Result<(), Box<dyn Error>> {
    let opts = parse_args();

    if !opts.follow {
        let entries = klog(opts.clear).unwrap();
        for entry in entries {
            println!("{}", entry)
        }
    } else {
        let log_timestamps_enabled = klog_timestamps_enabled()?;

        // ensure timestamps in logs
        if !log_timestamps_enabled {
            eprintln!("WARNING: Timestamps are disabled but tailing/following logs (as you've requested) requires them.");
            eprintln!("Aboring program.");
            eprintln!("You can enable timestamps by running the following: ");
            eprintln!("  echo Y > /sys/module/printk/parameters/time");
            return Err(RMesgError::KLogTimestampsDisabled.into());
        }

        let mut entries = KLogEntries::with_options(opts.clear, SUGGESTED_POLL_INTERVAL)?;

        while let Some(entry) = entries.try_next().await? {
            println!("{}", entry);
        }
    }

    Ok(())
}

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Box<dyn Error>> {
    let opts = parse_args();

    if !opts.follow {
        let entries = klog(opts.clear).unwrap();
        for entry in entries {
            println!("{}", entry)
        }
    } else {
        let log_timestamps_enabled = klog_timestamps_enabled()?;

        // ensure timestamps in logs
        if !log_timestamps_enabled {
            eprintln!("WARNING: Timestamps are disabled but tailing/following logs (as you've requested) requires them.");
            eprintln!("Aboring program.");
            eprintln!("You can enable timestamps by running the following: ");
            eprintln!("  echo Y > /sys/module/printk/parameters/time");
            return Err(RMesgError::KLogTimestampsDisabled.into());
        }

        let entries = KLogEntries::with_options(opts.clear, SUGGESTED_POLL_INTERVAL)?;

        for maybe_entry in entries {
            let entry = maybe_entry?;
            println!("{}", entry);
        }
    }

    Ok(())
}

fn parse_args() -> Options {
    let matches = App::new("rmest: A 'dmesg' port onto Rust")
        .version("0.2.0")
        .author("Archis Gore <archis@polyverse.com>")
        .about(
            "Reads (and prints) the kernel log buffer. Does not support all dmesg options (yet).",
        )
        .arg(
            Arg::with_name("follow")
                .short("f")
                .help("When specified, follows logs (like tail -f)"),
        )
        .arg(
            Arg::with_name("clear")
                .short("c")
                .help("Clear ring buffer after printing"),
        )
        .get_matches();

    let follow = !matches!(matches.occurrences_of("follow"), 0);
    let clear = !matches!(matches.occurrences_of("clear"), 0);

    Options { follow, clear }
}
