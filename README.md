# semver-bump

A tool for bumping version numbers in a semantic-version-compatible way, designed to be used in a shell scripting context. It takes the previous version number as input from `stdin`, bumps the segment you requested to be bumped, and emits the result to `stdout` with no other noise. It also handles bumping pre-release aka build identifiers as well, so you can increment `3.0.0-alpha.1` to `3.0.0-alpha.2`.

## Usage

There are four commands. The `prerelease` and `build` commands take an optional replacement identifier string parameter.

```text
> semver-bump help

Read a semver-compliant version number from stdin and bump the number as requested, writing the result to stdout

Usage: semver-bump <COMMAND>

Commands:
  major       Bump the major version number for a breaking change
  minor       Bump the minor version number for a new feature
  patch       Bump the patch version number for a bug fix
  prerelease  Bump any version number at the end of a pre-release identifier
  build       Bump any version number at the end of a build identifier
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## Examples

The use case I had in mind was automatic version bumping and tagging. Here we bump the version number of semver-bump itself:

```shell
#!/usr/bin/env bash
set -e
old=$(tomato get package.version Cargo.toml)
version=$(echo "$old" | cargo run -- "$1")
tomato set package.version "$version" Cargo.toml
cargo check
git commit Cargo.toml Cargo.lock -m "v$version"
git tag "$version"
echo "Release tagged for version $version"
```

A nearly identical version of this is in the justfile for this repo.

Here are some examples of the prerelease bumping behavior. There are some restrictions on what characters are allowed in the semver prerelease identifiers, and the semver crate's implementation is stricter than some.

```shell
> echo 1.2.3-alpha.4 | semver-bump prerelease
1.2.3-alpha.5

> echo 1.2.3-ceti-alpha-4 | semver-bump prerelease
1.2.3-ceti-alpha-5

> echo 1.2.3-ceti-alpha-5 | semver-bump prerelease beta
1.2.3-beta-1

> echo 1.0.0 | semver-bump prerelease
Error: The current version does not have a prerelease suffix and you did not provide one.

> echo 1.0.0 | semver-bump prerelease +illegal+
Error: unexpected character in pre-release identifier
```

## LICENSE

This code is licensed via [the Parity Public License.](https://paritylicense.com) This license requires people who build on top of this source code to share their work with the community, too. See the license text for details.
