//! A tool that I wrote because all the other ones weren't quite perfect.
//! This is a very simple wrapper around the semver crate that behaves
//! exactly as a need a version-bumping tool to behave, and that is built
//! and released in a way that makes it convenient to use in Github workflows.

#![deny(future_incompatible, clippy::unwrap_used)]
#![warn(rust_2018_idioms, trivial_casts)]

use std::str::FromStr;

use anyhow::anyhow;
use clap::{Parser, Subcommand};
use semver::{BuildMetadata, Prerelease, Version};

// Valid separators between the pre-release number
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

/// Increment the passed-in separator plus maybe-number.
fn increment_identifier(suffix: &str) -> anyhow::Result<String> {
    eprintln!("entering increment with {suffix}");
    let mut characters = suffix.chars().peekable();

    if let Some(maybe_sep) = characters.peek() {
        eprintln!("we have a maybe_sep: {maybe_sep}");
        if SEPARATORS.contains(maybe_sep) {
            eprintln!("our maybe sep is a sep");
            let separator = characters.next().expect("but we just checked this character");
            let remainder: String = characters.collect();
            let number = remainder.parse::<u64>()?;
            eprintln!("{remainder} parsed as {number}");
            return Ok(format!("{separator}{}", number + 1));
        } else if maybe_sep.is_ascii_digit() {
            let number = suffix.parse::<u64>()?;
            // preserve lack of separator
            return Ok(format!("{}", number + 1));
        }
    }
    Ok(format!("{suffix}.1"))

    /*

    if let Some(idx) = suffix.rfind(SEPARATORS) {
        // This is the version that panics, but we're trusting the result we
        // just got from the rfind.
        let split = suffix.split_at(idx);
        let mut characters = split.1.chars(); // well, code points
        eprintln!("{idx} {} {}", split.0, split.1);
        let separator = characters.next().unwrap_or('.');
        let possible_number: String = characters.collect();
        let buildnum = possible_number.parse::<u64>()?;
        let next = format!("{}{separator}{}", split.0, buildnum + 1);
        Ok(next)
    } else if let Ok(buildnum) = suffix.parse::<u64>() {
        // Try treating the whole thing as a number.
        Ok(format!("{}", buildnum + 1))
    } else {
        Ok(format!("{suffix}.1"))
    }
    */
}

/// Update the prerelease identifier for this version number.
/// If we don't have an existing identifier, we add one.
/// If we have an existing identifier that matches a passed-in tag, we increment.
/// If we have an existing identifier and no passed-in tag, we increment existing.
/// If we have no existing identifier and no tag, we report an input error to the user.
fn prerelease(previous: &Version, tag: &str) -> anyhow::Result<Version> {
    let mut next = Version::new(previous.major, previous.minor, previous.patch);
    if tag.is_empty() && !previous.pre.is_empty() {
        if let Some(idx) = previous.pre.rfind(SEPARATORS) {
            let split = previous.pre.split_at(idx);
            let incremented = increment_identifier(split.1)?;
            let full = format!("{}{incremented}", split.0);
            next.pre = Prerelease::from_str(full.as_str())?;
            Ok(next)
        } else {
            match increment_identifier(previous.pre.to_string().as_str()) {
                Ok(v) => {
                    next.pre = Prerelease::from_str(v.as_str())?;
                    Ok(next)
                }
                Err(_) => {
                    let pretag = format!("{}.1", previous.build);
                    next.pre = Prerelease::from_str(pretag.as_str())?;
                    Ok(next)
                }
            }
        }
    } else if !tag.is_empty() && previous.pre.starts_with(tag) {
        let remainder = previous.pre.to_string().replace(tag, "");
        let incremented = increment_identifier(remainder.as_str())?;
        let full = format!("{tag}{incremented}");
        next.pre = Prerelease::from_str(full.as_str())?;
        Ok(next)
    } else if !tag.is_empty() {
        let pretag = format!("{tag}.1");
        next.pre = Prerelease::from_str(pretag.as_str())?;
        Ok(next)
    } else if !previous.pre.is_empty() {
        let incremented = increment_identifier(previous.pre.to_string().as_str())?;
        next.pre = Prerelease::from_str(incremented.as_str())?;
        Ok(next)
    } else {
        Err(anyhow!(
            "The current version does not have a prerelease suffix and you did not provide one."
        ))
    }
}

/// This works just like prerelease, only it operates on the build segment.
fn build(previous: &Version, tag: &str) -> anyhow::Result<Version> {
    let mut next = Version::new(previous.major, previous.minor, previous.patch);
    next.pre = previous.pre.clone();

    eprintln!("incrementing {}", previous.build);

    if tag.is_empty() && !previous.build.is_empty() {
        if let Some(idx) = previous.build.rfind(SEPARATORS) {
            let split = previous.build.split_at(idx);
            let incremented = increment_identifier(split.1)?;
            let full = format!("{}{incremented}", split.0);
            next.build = BuildMetadata::from_str(full.as_str())?;
            Ok(next)
        } else {
            match increment_identifier(previous.build.to_string().as_str()) {
                Ok(v) => {
                    next.build = BuildMetadata::from_str(v.as_str())?;
                    Ok(next)
                }
                Err(_) => {
                    let buildid = format!("{}.1", previous.build);
                    next.build = BuildMetadata::from_str(buildid.as_str())?;
                    Ok(next)
                }
            }
        }
    } else if !tag.is_empty() && previous.build.starts_with(tag) {
        let remainder = previous.build.to_string().replace(tag, "");
        let incremented = increment_identifier(remainder.as_str())?;
        let full = format!("{tag}{incremented}");
        next.build = BuildMetadata::from_str(full.as_str())?;
        Ok(next)
    } else if !tag.is_empty() {
        let buildid = format!("{tag}.1");
        next.build = BuildMetadata::from_str(buildid.as_str())?;
        Ok(next)
    } else if !previous.build.is_empty() {
        eprintln!("in the non-empty case");
        Ok(next)
    } else {
        Err(anyhow!(
            "The current version does not have a build identifier and you did not provide one."
        ))
    }
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
        let next = prerelease(&input, "").expect("we expected this to work");
        assert_eq!(
            next.pre,
            Prerelease::new("ceti-alpha-5").expect("test data must be valid semver")
        );
        assert_eq!(next.to_string(), "1.2.3-ceti-alpha-5".to_string());

        let input = Version::parse("1.2.3-ceti-alpha.4").expect("test data must be valid semver");
        let next = prerelease(&input, "").expect("we expected this to work");
        assert_eq!(next.to_string(), "1.2.3-ceti-alpha.5".to_string());
    }

    #[test]
    fn build_bump() {
        let input = Version::parse("1.2.3-four+4").expect("test data must be valid semver");
        let next = build(&input, "").expect("we expected this to work");
        assert_eq!(next.to_string(), "1.2.3-four+5".to_string());
        let input = Version::parse("1.2.3-ceti-alpha+4").expect("test data must be valid semver");
        let next = build(&input, "").expect("we expected this to work");
        assert_eq!(next.to_string(), "1.2.3-ceti-alpha+5".to_string());
    }
}
