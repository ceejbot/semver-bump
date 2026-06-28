_help:
    just -l

# Run tests using nextest
@test:
    cargo nextest run

# Format using nightly
@fmt:
    cargo +nightly fmt

# Clippy is our friend
@lint:
    cargo clippy --all-targets -- -D warnings

# Run the same checks we run in ci.
@ci: test lint
    cargo +nightly fmt --check

# Install required tools
setup:
    brew tap ceejbot/tap
    brew install fzf tomato cargo-nextest
    rustup install nightly

# Tag a new version for release, using itself.
version +V="patch":
    #!/usr/bin/env bash
    set -e
    if [[ ! -z $(git status --untracked-files=no --porcelain) ]]; then
    	echo "Git working directory has uncommitted changes! Exiting."
    	exit 1
    fi
    old=$(tomato get package.version Cargo.toml)
    version=$(cargo run -- {{ V }} "$old")
    tomato set package.version "$version" Cargo.toml &> /dev/null
    cargo check
    git commit Cargo.toml Cargo.lock -m "v$version"
    git tag "v$version"
    echo "Release tagged for version $version"
