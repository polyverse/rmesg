# rmesg

[![Build Status](https://travis-ci.org/polyverse/rmesg.svg?branch=master)](https://travis-ci.org/polyverse/rmesg)

A 'dmesg' implementation in Rust

## As a command-line utility

### Cargo Install

```.bash
cargo install rmesg
```

### Usage

```.bash
rmesg --help
rmest: A 'dmesg' port onto Rust 0.2.0
Archis Gore <archis@polyverse.com>
Reads (and prints) the kernel log buffer. Does not support all dmesg options (yet).

USAGE:
    rmesg [FLAGS]

FLAGS:
    -c               Clear ring buffer after printing
    -f               When specified, follows logs (like tail -f)
    -h, --help       Prints help information
    -V, --version    Prints version information
```

## As a Crate

This mainly serves as a crate, but if compiled, will become a
simple executable that will dump kernel log buffer contents onto
the console.

The value of this is programmatic access to kernel buffer from Rust
programs, and a Rust-based `klogctl` implementation.

### Depend on the rmesg crate

Include it in Cargo.toml:

```.toml
[dependencies]
rmesg = "0.6.0"
```

### Reading the entire buffer as a string

To read the kernel message buffer as a String (the string will have line-breaks you'll have to make sense of):

```.rust
    use rmesg::rmesg;

    let log_result = rmesg(false);
    assert!(log_result.is_ok(), "Failed to call rmesg");

    let logs = log_result.unwrap();
    assert!(logs.len() > 0, "Should have non-empty logs");
```

The parameter sole bool parameter tells the rmesg call whether to clear the buffer when read, or to preserve it. When preserved,
subsequent calls will get repeated lines. In order to read line-by-line without clearing the buffer, use the second method described below.

### Reading the buffer line-by-line

However must useful is being able to read the kernel logs line-by-line:

```.rust
    use rmesg::{kernel_log_timestamps_enable, RMesgLinesIterator, SUGGESTED_POLL_INTERVAL};

    // Enable timestamps in kernel log lines if not already enabled - otherwise the iterator will
    // ignore all lines and get stuck.
    let enable_timestamp_result = kernel_log_timestamps_enable(true);
    assert!(enable_timestamp_result.is_ok());

    // Don't clear the buffer. Poll every second.
    let iterator_result = RMesgLinesIterator::with_options(false, SUGGESTED_POLL_INTERVAL);
    assert!(iterator_result.is_ok());

    let iterator = iterator_result.unwrap();

    for line in iterator {
        // Do stuff
    }
```
