#!/bin/bash
# lint.sh - Run cargo clippy in all packages

set -e

PACKAGES=(vass-reach-lib vass-reach vass-reach-testing vass-reach-playground)

for pkg in "${PACKAGES[@]}"; do
    echo "Linting package: $pkg"
    cargo clippy --manifest-path "packages/$pkg/Cargo.toml" --all-targets --all-features -- -D warnings
done
