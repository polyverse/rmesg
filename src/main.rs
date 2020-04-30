use clap::{App, Arg};
use rmesg::{kernel_log_timestamps_enabled, rmesg, RMesgLinesIterator, SUGGESTED_POLL_INTERVAL};
use std::process;

#[derive(Debug)]
struct Options {
    follow: bool,
    clear: bool,
}

fn main() {
    let opts = parse_args();

    if !opts.follow {
        println!("{}", rmesg(opts.clear).unwrap())
    } else {
        let log_timestamps_enabled = match kernel_log_timestamps_enabled() {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Unable to check whether kernel log timestamps are enabled. Unable to follow/tail logs. Error: {:?}", e);
                process::exit(1);
            }
        };

        // ensure timestamps in logs
        if !log_timestamps_enabled {
            eprintln!("WARNING: Timestamps are disabled but tailing/following logs (as you've requested) requires them.");
            eprintln!("You may see no output (lines without timestamps are ignored).");
            eprintln!("You can enable timestamps by running the following: ");
            eprintln!("  echo Y > /sys/module/printk/parameters/time");
        }

        let lines = match RMesgLinesIterator::with_options(opts.clear, SUGGESTED_POLL_INTERVAL) {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "Unable to get an iterator over kernel log messages: {:?}",
                    e
                );
                process::exit(1);
            }
        };
        for maybe_line in lines {
            match maybe_line {
                Ok(line) => println!("{}", line),
                Err(e) => {
                    eprintln!("Error when iterating over kernel log messages: {:?}", e);
                    process::exit(1);
                }
            }
        }
    }
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

    let follow = match matches.occurrences_of("follow") {
        0 => false,
        _ => true,
    };

    let clear = match matches.occurrences_of("clear") {
        0 => false,
        _ => true,
    };

    Options { follow, clear }
}
