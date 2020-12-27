/// rmesg - a rust-based dmesg implementation.
/// This CLI builds on top of the eponymous crate and provides a command-line utility.
///
use clap::{App, Arg};
use std::error::Error;

#[cfg(feature = "async")]
use futures_util::stream::TryStreamExt;

#[derive(Debug)]
struct Options {
    follow: bool,
    clear: bool,
    raw: bool,
    backend: rmesg::Backend,
}

#[cfg_attr(feature = "async", tokio::main(flavor = "current_thread"))]
#[cfg(feature = "async")]
async fn main() -> Result<(), Box<dyn Error>> {
    let opts = parse_args();

    if !opts.follow {
        nofollow(opts);
    } else {
        let mut entries = rmesg::logs_iter(opts.backend, opts.clear, opts.raw).await?;

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
        nofollow(opts);
    } else {
        let entries = rmesg::logs_iter(opts.backend, opts.clear, opts.raw)?;
        for maybe_entry in entries {
            let entry = maybe_entry?;
            println!("{}", entry);
        }
    }

    Ok(())
}

fn nofollow(opts: Options) {
    if opts.raw {
        let raw = rmesg::logs_raw(opts.backend, opts.clear).unwrap();
        print!("{}", raw)
    } else {
        let entries = rmesg::log_entries(opts.backend, opts.clear).unwrap();
        for entry in entries {
            println!("{}", entry)
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
        .arg(
            Arg::with_name("raw")
                .short("r")
                .help("Print raw data as it came from the source backend."),
        )
        .arg(
            Arg::with_name("backend")
                .short("b")
                .takes_value(true)
                .possible_values(&["klogctl", "devkmsg"])
                .help("Select backend from where to read the logs. klog is the syslog/klogctl system call through libc. kmsg is the /dev/kmsg file."),
        )
        .get_matches();

    let follow = !matches!(matches.occurrences_of("follow"), 0);
    let clear = !matches!(matches.occurrences_of("clear"), 0);
    let raw = !matches!(matches.occurrences_of("raw"), 0);
    let backend = match matches.value_of("backend") {
        None => rmesg::Backend::Default,
        Some("klogctl") => rmesg::Backend::KLogCtl,
        Some("devkmsg") => rmesg::Backend::DevKMsg,
        Some(v) => panic!("Something went wrong. Possible values for backend were not restricted by the CLI parser and this value slipped through somehow: {}", v),
    };

    Options {
        follow,
        clear,
        raw,
        backend,
    }
}
