# VASS Reachability Tool

A work in progress tool for solving VASS reachability based on recent results on regular separability for VASS reachability languages.

## Requirements

- [Rust](https://www.rust-lang.org/) (and Cargo) 1.79.0 and above

## Usage

Currently the project only includes tests which can be run with:

```sh
cargo test
```

Or with `RUST_BACKTRACE=1 cargo test --release -- --test-threads=1 --nocapture` to get fast, non overlapping tests with debug output.

## Authors and Contact

This is a project by the [Institute of Theoretical Computer Science](https://www.tcs.cs.tu-bs.de) of the Technical University Brunswick.