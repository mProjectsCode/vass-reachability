all: run

run: 
	@cargo run --release

test:
	@RUST_BACKTRACE=full cargo test -- --test-threads=1 --nocapture

test-r:
	@RUST_BACKTRACE=1 cargo test --release --no-fail-fast -- --test-threads=1 --nocapture