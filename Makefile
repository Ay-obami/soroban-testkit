.PHONY: check fmt lint test doc audit build clean

check: fmt lint test

build:
	cargo build --workspace

fmt:
	cargo fmt --all --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

audit:
	cargo audit

clean:
	cargo clean
