_help:
	just -l

# Run tests using nextest
test:
	cargo nextest run

# Format using nightly
fmt:
	cargo +nightly fmt

# Install required tools
setup:
	brew tap ceejbot/tap
	brew install fzf tomato cargo-nextest
	rustup install nightly

# Tag a new version for release, using itself.
tag-release +V="patch":
	#!/usr/bin/env bash
	set -e
	if [[ ! -z $(git status --untracked-files=no --porcelain) ]]; then
		echo "Git working directory has uncommitted changes! Exiting."
		exit 1
	fi
	version=$(echo $(tomato get package.version Cargo.toml) | cargo run -- {{V}})
	tomato set package.version "$version" Cargo.toml &> /dev/null
	cargo check
	git commit Cargo.toml Cargo.lock -m "v$version"
	git tag "$version"
	echo "Release tagged for version $version"
