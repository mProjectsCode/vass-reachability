#!/bin/bash
# format.sh - Run cargo fmt in all packages

set -e

PACKAGES=(vass-reach-lib vass-reach vass-reach-testing vass-reach-playground)

for pkg in "${PACKAGES[@]}"; do
    echo "Formatting package: $pkg"
    cargo fmt --manifest-path "packages/$pkg/Cargo.toml"
done
