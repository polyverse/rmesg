![Build Status](https://github.com/polyverse/rmesg/workflows/Build%20Status/badge.svg)

# rmesg

A 'dmesg' implementation in Rust

## As a command-line utility

### Obtain the latest release binary

```.bash
wget https://github.com/polyverse/rmesg/releases/latest/download/rmesg
chmod a+x ./rmesg
# Optionally move to a stable location
mv ./rmesg /usr/local/bin
```

### Cargo Install

```.bash
cargo install rmesg
```

### Usage

```.bash
rmesg: A 'dmesg' port onto Rust 1.0.0
Archis Gore <archis@polyverse.com>
Reads (and prints) the kernel log buffer. Does not support all dmesg options (yet).

USAGE:
    rmesg [FLAGS] [OPTIONS]

FLAGS:
    -c               Clear ring buffer after printing (only when using klogctl)
    -f               When specified, follows logs (like tail -f)
    -h, --help       Prints help information
    -r               Print raw data as it came from the source backend.
    -V, --version    Prints version information

OPTIONS:
    -b <backend>        Select backend from where to read the logs. klog is the syslog/klogctl system call through libc.
                        kmsg is the /dev/kmsg file. [possible values: klogctl, devkmsg]
```

## As a Crate

The real value of this crate is  programmatic access to kernel buffer from Rust
programs, allowing a `dmesg` that can be consumed programmatically.

The complete API can be found in the `main.rs` file which uses the sync/async versions of the APIs, both single-shot and iterative.

### Depend on the rmesg crate

Include it in Cargo.toml:

```.toml
[dependencies]
rmesg = "1.0.0"
```

Suppots two features:

* `async` - Exposes asynchronous Stream API
* `sync` - Exposes synchronous Iterator API

### Reading the buffer single-shot (non-blocking)

*NOTE: Reading single-shot is the same interface for sync or async*

```.rust
    use rmesg;

    // Read all logs as one big string with line-breaks
    let raw = rmesg::logs_raw(opts.backend, opts.clear).unwrap();
    print!("{}", raw)

    // Read logs as a Vec of Entry'ies (`Vec<Entry>`)
    // and can be processed entry-by-entry
    let entries = rmesg::log_entries(opts.backend, opts.clear).unwrap();
    for entry in entries {
        println!("{}", entry)
    }
```

### Indefinitely iterating

With feature `sync` (i.e. synchronous), provides an Iterator over Result<Entry, RMesgError>.

```.rust
    use rmesg;

    let entries = rmesg::logs_iter(opts.backend, opts.clear, opts.raw)?;
    for maybe_entry in entries {
        let entry = maybe_entry?;
        println!("{}", entry);
    }
```

With feature `async` (i.e. asynchronous), provides a Stream over Result<Entry, RMesgError>.

```.rust
    use rmesg;

    // given that it's a stream over Result's, use the conveniences provided to us
    use futures_util::stream::TryStreamExt;

    let mut entries = rmesg::logs_stream(opts.backend, opts.clear, opts.raw).await?;

    while let Some(entry) = entries.try_next().await? {
        println!("{}", entry);
    }
```
