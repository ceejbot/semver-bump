//! Yet another semver bumping cli because all the other ones weren't quite perfect.
//! This is a very simple wrapper around the semver crate that behaves
//! exactly as a need a version-bumping tool to behave, and that is built
//! and released in a way that makes it convenient to use in Github workflows.
//! It handles incrementing or replacing pre-release and build identifiers as well
//! as the usual major.minor.patch numbers.

#![deny(future_incompatible, clippy::unwrap_used)]
#![warn(rust_2018_idioms, trivial_casts)]

use std::fmt::Display;
use std::str::FromStr;

use anyhow::anyhow;
use clap::{Parser, Subcommand};
use semver::{BuildMetadata, Prerelease, Version};

// Valid separators between the pre-release and its number;
// no separator at all is also valid.
const SEPARATORS: [char; 2] = ['.', '-'];

#[derive(Parser, Debug)]
#[clap(name = "semver-bump", version)]
/// Read a semver-compliant version number from stdin and bump the number as requested,
/// writing the result to stdout.
pub struct Args {
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    /// Bump the major version number for a breaking change.
    Major,
    /// Bump the minor version number for a new feature.
    Minor,
    /// Bump the patch version number for a bug fix.
    Patch,
    #[command(about = "Bump any version number at the end of a pre-release identifier", long_about)]
    /// This command handles incrementing prerelease identifiers of the form
    /// `<id><sep><#>`. If no pre-release identifier is present in the input, one
    /// is added with count 1. The command defaults to `.` as a separator, but respects
    /// `.` and `-` as valid separators.
    Prerelease {
        /// The pre-release identifier to use; optional if you're re-using the existing identifier.
        /// Must contain only alphanumeric characters plus any of the valid separator characters.
        identifier: Option<String>,
    },
    /// Bump any version number at the end of a build identifier.
    Build {
        // An optional build identifier to use if you want to add one to a version,
        // or to replace an existing build identifier. Behaves like bumping a prerelease.
        identifier: Option<String>,
    },
}

/// Increment the major version.
fn major(previous: &Version) -> Version {
    Version::new(previous.major + 1, 0, 0)
}

/// Increment the minor version.
fn minor(previous: &Version) -> Version {
    Version::new(previous.major, previous.minor + 1, 0)
}

/// Increment the patch version.
fn patch(previous: &Version) -> Version {
    Version::new(previous.major, previous.minor, previous.patch + 1)
}

trait Incrementable: Display {
    fn create_new(input: String) -> anyhow::Result<Box<Self>>;
}

impl Incrementable for Prerelease {
    fn create_new(input: String) -> anyhow::Result<Box<Prerelease>> {
        match Prerelease::from_str(input.as_str()) {
            Ok(v) => Ok(Box::new(v)),
            Err(e) => Err(anyhow::Error::from(e)),
        }
    }
}

impl Incrementable for BuildMetadata {
    fn create_new(input: String) -> anyhow::Result<Box<BuildMetadata>> {
        match BuildMetadata::from_str(input.as_str()) {
            Ok(v) => Ok(Box::new(v)),
            Err(e) => Err(anyhow::Error::from(e)),
        }
    }
}

/// Increment the passed-in separator plus maybe-number.
fn increment_identifier(suffix: &str) -> anyhow::Result<String> {
    let mut characters = suffix.chars().peekable();

    if let Some(maybe_sep) = characters.peek() {
        if SEPARATORS.contains(maybe_sep) {
            let separator = characters.next().expect("but we just checked this character!");
            let remainder: String = characters.collect();
            let number = remainder.parse::<u64>()?;
            return Ok(format!("{separator}{}", number + 1));
        } else if maybe_sep.is_ascii_digit() {
            let number = suffix.parse::<u64>()?;
            // preserve lack of separator
            return Ok(format!("{}", number + 1));
        }
    }
    Ok(format!("{suffix}.1"))
}

/// Update the identifier for this version number.
/// If we don't have an existing identifier, we add one.
/// If we have an existing identifier that matches a passed-in tag, we increment.
/// If we have an existing identifier and no passed-in tag, we increment existing.
/// If we have no existing identifier and no tag, we report an input error to the user.
fn increment<T: Incrementable>(input: &T, tag: &str) -> anyhow::Result<Box<T>> {
    let previous = input.to_string();

    let identifier = if tag.is_empty() && !previous.is_empty() {
        if let Some(idx) = previous.rfind(SEPARATORS) {
            let split = previous.split_at(idx);
            let incremented = increment_identifier(split.1)?;
            format!("{}{incremented}", split.0)
        } else {
            match increment_identifier(previous.to_string().as_str()) {
                Ok(v) => v,
                Err(_) => {
                    format!("{}.1", previous)
                }
            }
        }
    } else if !tag.is_empty() && tag != previous {
        let last = tag.chars().last().unwrap_or_default();
        if last.is_ascii_digit() {
            tag.to_owned()
        } else {
            format!("{tag}.1")
        }
    } else if !tag.is_empty() && previous.starts_with(tag) {
        let remainder = previous.to_string().replace(tag, "");
        let incremented = increment_identifier(remainder.as_str())?;
        format!("{tag}{incremented}")
    } else if !tag.is_empty() {
        format!("{tag}.1")
    } else if !previous.is_empty() {
        increment_identifier(previous.to_string().as_str())?
    } else {
        return Err(anyhow!(
            "The current version does not have a prerelease suffix and you did not provide one."
        ));
    };

    let next = T::create_new(identifier)?;
    Ok(next)
}

/// Replace or add a prerelease identifier, or increment the number at the
/// end of an existing prerelease identifier.
fn prerelease(previous: &Version, tag: &str) -> anyhow::Result<Version> {
    let mut next = Version::new(previous.major, previous.minor, previous.patch);
    let identifier = increment(&previous.pre, tag)?;
    next.pre = *identifier;
    Ok(next)
}

/// This works just like prerelease, only it operates on the build segment.
fn build(previous: &Version, tag: &str) -> anyhow::Result<Version> {
    let mut next = Version::new(previous.major, previous.minor, previous.patch);
    next.pre = previous.pre.clone();
    let identifier = increment(&previous.build, tag)?;
    next.build = *identifier;
    Ok(next)
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut buffer = String::new();
    let stdin = std::io::stdin();
    stdin.read_line(&mut buffer)?;
    let trimmed = buffer.trim();
    let previous = Version::parse(trimmed)?;

    let result = match args.cmd {
        Command::Major => major(&previous),
        Command::Minor => minor(&previous),
        Command::Patch => patch(&previous),
        Command::Prerelease { identifier } => {
            let tag = identifier.unwrap_or_default();
            prerelease(&previous, tag.as_str())?
        }
        Command::Build { identifier } => {
            let tag = identifier.unwrap_or_default();
            build(&previous, tag.as_str())?
        }
    };
    println!("{result}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use semver::Version;

    use super::*;

    #[test]
    fn major_bump() {
        let input = Version::parse("1.0.0").expect("test data must be valid semver");
        let next = major(&input);
        assert_eq!(next.major, input.major + 1);
        let input = Version::parse("3.3.3").expect("test data must be valid semver");
        let next = major(&input);
        assert_eq!(next.major, input.major + 1);
        assert_eq!(next.minor, 0);
        assert_eq!(next.patch, 0);
        let input = Version::parse("747.341.321-alpha1").expect("test data must be valid semver");
        let next = major(&input);
        assert!(next.pre.is_empty());
    }

    #[test]
    fn minor_bump() {
        // boring but I will write a test
        let input = Version::parse("1.2.3").expect("test data must be valid semver");
        let next = minor(&input);
        assert_eq!(next.major, input.major);
        assert_eq!(next.minor, input.minor + 1);
        assert_eq!(next.patch, 0);
    }

    #[test]
    fn patch_bump() {
        // boring but I will write a test
        let input = Version::parse("1.2.3").expect("test data must be valid semver");
        let next = patch(&input);
        assert_eq!(next.major, input.major);
        assert_eq!(next.minor, input.minor);
        assert_eq!(next.patch, input.patch + 1);
    }

    #[test]
    fn prerelease_adding() {
        let input = Version::parse("1.0.0").expect("test data must be valid semver");
        let next = prerelease(&input, "alpha").expect("we expected the prerelease bump to work");
        assert_eq!(next.major, input.major);
        assert_eq!(
            next.pre,
            Prerelease::new("alpha.1").expect("test data must be valid semver")
        );
        let input = Version::parse("1.0.0-alpha").expect("test data must be valid semver");
        let next = prerelease(&input, "").expect("we expected the prerelease bump to work");
        assert_eq!(next.major, input.major);
        assert_eq!(
            next.pre,
            Prerelease::new("alpha.1").expect("test data must be valid semver")
        );
        let input = Version::parse("1.0.0-alpha").expect("test data must be valid semver");
        let next = prerelease(&input, "alpha").expect("we expected the prerelease bump to work");
        assert_eq!(next.major, input.major);
        assert_eq!(
            next.pre,
            Prerelease::new("alpha.1").expect("test data must be valid semver")
        );
    }

    #[test]
    fn prerelease_bumping() {
        let input = Version::parse("1.0.0-alpha.1").expect("test data must be valid semver");
        let next = prerelease(&input, "").expect("we expected the prerelease bump to work");
        assert_eq!(next.major, input.major);
        assert_eq!(
            next.pre,
            Prerelease::new("alpha.2").expect("test data must be valid semver")
        );
        let next = prerelease(&input, "beta").expect("we expected the prerelease bump to work");
        assert_eq!(
            next.pre,
            Prerelease::new("beta.1").expect("test data must be valid semver")
        );
        let input = Version::parse("1.0.0-1").expect("test data must be valid semver");
        let next = prerelease(&input, "").expect("we expected the prerelease bump to work");
        assert_eq!(next.pre, Prerelease::new("2").expect("test data must be valid semver"));
    }

    #[test]
    fn prerelease_error_cases() {
        let input = Version::parse("1.0.0").expect("test data must be valid semver");
        prerelease(&input, "").expect_err("we expected an error from this call");
        prerelease(&input, "+illegal+").expect_err("we expected an error from this call");
        let input = Version::parse("1.0.0-alpha.four").expect("test data must be valid semver");
        prerelease(&input, "").expect_err("we expected an error from this call");
    }

    #[test]
    fn separator_detection() {
        let input = Version::parse("1.2.3-ceti-alpha-4").expect("test data must be valid semver");
        let next = prerelease(&input, "").expect("we expected prerelease() to work");
        assert_eq!(
            next.pre,
            Prerelease::new("ceti-alpha-5").expect("test data must be valid semver")
        );
        assert_eq!(next.to_string(), "1.2.3-ceti-alpha-5".to_string());

        let input = Version::parse("1.2.3-ceti-alpha.4").expect("test data must be valid semver");
        let next = prerelease(&input, "").expect("we expected prerelease() to work");
        assert_eq!(next.to_string(), "1.2.3-ceti-alpha.5".to_string());
    }

    #[test]
    fn build_bump() {
        let input = Version::parse("1.2.3-four+4").expect("test data must be valid semver");
        let next = build(&input, "").expect("we expected build() to work");
        assert_eq!(next.to_string(), "1.2.3-four+5".to_string());
        let input = Version::parse("1.2.3-ceti-alpha+4").expect("test data must be valid semver");
        let next = build(&input, "").expect("we expected build() to work");
        assert_eq!(next.to_string(), "1.2.3-ceti-alpha+5".to_string());
    }

    #[test]
    fn passing_numbers_in() {
        let input = Version::parse("1.2.3-four+4").expect("test data must be valid semver");
        let next = prerelease(&input, "beta.2").expect("we expected prerelease() to work");
        assert_eq!(next.to_string(), "1.2.3-beta.2");
        let input = Version::parse("1.2.3-four+4").expect("test data must be valid semver");
        let next = build(&input, "7").expect("we expected build() to work");
        assert_eq!(next.to_string(), "1.2.3-four+7");
    }
}
