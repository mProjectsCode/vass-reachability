all: run

run: 
	@cargo run --release

test:
	@RUST_BACKTRACE=1 cargo test --release -- --test-threads=1 --nocapture