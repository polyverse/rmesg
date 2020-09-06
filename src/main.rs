/// rmesg - a rust-based dmesg implementation.
/// This CLI builds on top of the eponymous crate and provides a command-line utility.
///
use rmesg::{kernel_log_timestamps_enabled, rmesg, RMesgLinesIterator, SUGGESTED_POLL_INTERVAL};
use std::process;
use seahorse::{App, Flag, FlagType, Context};
use std::env;

struct Options {
    follow: bool,
    clear: bool,
}

fn main() {
    parse_args();
}

fn parse_args() {
    let app = App::new("rmesg")
        .usage("cli [flags]")
        .version("0.2.0")
        .author("Archis Gore <archis@polyverse.com>")
        .description (
            "Reads (and prints) the kernel log buffer. Does not support all dmesg options (yet).",
        )
        .action(sanitize_args)
        .flag(Flag::new("follow", FlagType::Bool)
            .alias("f")
            .usage("When specified, follows logs (like tail -f)"))
        .flag(Flag::new("clear", FlagType::Bool)
            .alias("c")
            .usage("Clear ring buffer after printing"));

    let args: Vec<String> = env::args().collect();
    app.run(args);
}

fn sanitize_args(c: &Context) {
    let opts = Options{
        follow: c.bool_flag("follow"),
        clear: c.bool_flag("clear"),
    };

    do_rmesg(opts)
}

fn do_rmesg(opts: Options) {
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