
[![Build Status](https://travis-ci.org/polyverse/rmesg.svg?branch=master)](https://travis-ci.org/polyverse/rmesg)

# rmesg
A 'dmesg' implementation in Rust

This mainly serves as a crate, but if compiled, will become a
simple executable that will dump kernel log buffer contents onto
the console.

The value of this is programmatic access to kernel buffer from Rust
programs, and a Rust-based `klogctl` implementation.

