
[![Build Status](https://travis-ci.org/polyverse/rmesg.svg?branch=master)](https://travis-ci.org/polyverse/rmesg)

# rmesg
A 'dmesg' implementation in Rust

## As a Crate

This mainly serves as a crate, but if compiled, will become a
simple executable that will dump kernel log buffer contents onto
the console.

The value of this is programmatic access to kernel buffer from Rust
programs, and a Rust-based `klogctl` implementation.

### Including the crate:

Include it in Cargo.toml:

```.toml
[dependencies]
rmesg = "0.5.0"
```

### Reading the entire buffer as a string

To read the kernel message buffer as a String (the string will have line-breaks you'll have to make sense of):

```.rust
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
```

