//! Yet another semver bumping cli because all the other ones weren't quite perfect.
//! This is a very simple wrapper around the semver crate that behaves
//! exactly as a need a version-bumping tool to behave, and that is built
//! and released in a way that makes it convenient to use in Github workflows.
//! It handles incrementing or replacing pre-release and build identifiers as well
//! as the usual major.minor.patch numbers.

use std::fmt::Display;
use std::str::FromStr;

use anyhow::anyhow;
use clap::builder::Styles;
use clap::builder::styling::AnsiColor;
use clap::{Parser, Subcommand};
use semver::{BuildMetadata, Prerelease, Version};

// Valid separators between the pre-release and its number;
// no separator at all is also valid.
const SEPARATORS: [char; 2] = ['.', '-'];

#[derive(Parser, Debug)]
#[clap(name = "semver-bump", version, styles = v3_styles(), max_term_width = 100)]
/// Bump a semver-compliant version number and write the result to stdout.
pub struct Args {
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    /// Bump the major version number for a breaking change.
    Major {
        /// The version to bump.
        version: String,
    },
    /// Bump the minor version number for a new feature.
    Minor {
        /// The version to bump.
        version: String,
    },
    /// Bump the patch version number for a bug fix.
    Patch {
        /// The version to bump.
        version: String,
    },
    #[command(about = "Bump any version number at the end of a pre-release identifier", long_about)]
    /// This command handles incrementing prerelease identifiers of the form
    /// `<id><sep><#>`. If no pre-release identifier is present in the input, one
    /// is added with count 1. The command defaults to `.` as a separator, but respects
    /// `.` and `-` as valid separators.
    Prerelease {
        /// The version to bump.
        version: String,
        /// The optional pre-release identifier to use.
        /// Must contain only alphanumeric characters plus any of the valid separator characters.
        identifier: Option<String>,
    },
    /// Bump any version number at the end of a build identifier.
    Build {
        /// The version to bump.
        version: String,
        /// The optional build identifier to use.
        /// Must contain only alphanumeric characters plus any of the valid separator characters.
        identifier: Option<String>,
    },
}

/// I like my clap help styled the old way.
fn v3_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Yellow.on_default())
        .usage(AnsiColor::Green.on_default())
        .literal(AnsiColor::Green.on_default())
        .placeholder(AnsiColor::Green.on_default())
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

trait Incrementable: Display + Sized {
    fn create_new(input: String) -> anyhow::Result<Self>;
}

impl Incrementable for Prerelease {
    fn create_new(input: String) -> anyhow::Result<Self> {
        Ok(Prerelease::from_str(&input)?)
    }
}

impl Incrementable for BuildMetadata {
    fn create_new(input: String) -> anyhow::Result<Self> {
        Ok(BuildMetadata::from_str(&input)?)
    }
}

/// Increment the passed-in separator plus maybe-number.
fn increment_identifier(suffix: &str) -> anyhow::Result<String> {
    let mut characters = suffix.chars().peekable();

    if let Some(maybe_sep) = characters.peek() {
        if SEPARATORS.contains(maybe_sep) {
            let separator = characters.next().expect("but we just checked this character!");
            let remainder: String = characters.collect();
            // Check if remainder is numeric before parsing
            if let Ok(number) = remainder.parse::<u64>() {
                return Ok(format!("{separator}{}", number + 1));
            } else {
                // Non-numeric after separator, append .1
                return Ok(format!("{suffix}.1"));
            }
        } else if maybe_sep.is_ascii_digit() {
            // Check if entire suffix is numeric
            if let Ok(number) = suffix.parse::<u64>() {
                // preserve lack of separator
                return Ok(format!("{}", number + 1));
            }
        }
    }
    Ok(format!("{suffix}.1"))
}

/// Update the identifier for this version number. There are three cases plus an error:
/// - No tag given but one already exists: increment the number at the end of the existing one.
/// - A tag is given that the existing identifier already uses: increment its trailing number.
/// - A brand-new tag is given: use it as-is if it ends in a digit, otherwise start it at `.1`.
/// - No tag given and none exists: report an input error to the user.
fn increment<T: Incrementable>(input: &T, tag: &str) -> anyhow::Result<T> {
    let previous = input.to_string();

    let identifier = if tag.is_empty() {
        if previous.is_empty() {
            return Err(anyhow!(
                "The current version does not have a prerelease suffix and you did not provide one."
            ));
        }
        // No tag given: increment the number at the end of the existing identifier.
        if let Some(idx) = previous.rfind(SEPARATORS) {
            let (head, suffix) = previous.split_at(idx);
            format!("{head}{}", increment_identifier(suffix)?)
        } else {
            increment_identifier(&previous)?
        }
    } else if let Some(remainder) = previous.strip_prefix(tag) {
        // Existing identifier already uses this tag: increment its trailing number.
        format!("{tag}{}", increment_identifier(remainder)?)
    } else {
        // Brand-new tag: keep as-is if it already ends in a digit, else start at .1.
        if tag.chars().last().unwrap_or_default().is_ascii_digit() {
            tag.to_owned()
        } else {
            format!("{tag}.1")
        }
    };

    T::create_new(identifier)
}

/// Replace or add a prerelease identifier, or increment the number at the
/// end of an existing prerelease identifier.
fn prerelease(previous: &Version, tag: &str) -> anyhow::Result<Version> {
    let mut next = Version::new(previous.major, previous.minor, previous.patch);
    let identifier = increment(&previous.pre, tag)?;
    next.pre = identifier;
    next.build = previous.build.clone(); // Preserve build metadata
    Ok(next)
}

/// This works just like prerelease, only it operates on the build segment.
fn build(previous: &Version, tag: &str) -> anyhow::Result<Version> {
    let mut next = Version::new(previous.major, previous.minor, previous.patch);
    next.pre = previous.pre.clone();
    let identifier = increment(&previous.build, tag)?;
    next.build = identifier;
    Ok(next)
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let previous = match &args.cmd {
        Command::Major { version }
        | Command::Minor { version }
        | Command::Patch { version }
        | Command::Prerelease { version, .. }
        | Command::Build { version, .. } => Version::parse(version)?,
    };

    let result = match args.cmd {
        Command::Major { .. } => major(&previous),
        Command::Minor { .. } => minor(&previous),
        Command::Patch { .. } => patch(&previous),
        Command::Prerelease { identifier, .. } => {
            let tag = identifier.unwrap_or_default();
            prerelease(&previous, tag.as_str())?
        }
        Command::Build { identifier, .. } => {
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
        // Non-numeric suffixes are now handled gracefully by appending .1
        let input = Version::parse("1.0.0-alpha.four").expect("test data must be valid semver");
        let next = prerelease(&input, "").expect("should handle non-numeric gracefully");
        assert_eq!(next.to_string(), "1.0.0-alpha.four.1");
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
        // When replacing prerelease, build metadata is preserved
        assert_eq!(next.to_string(), "1.2.3-beta.2+4");
        let input = Version::parse("1.2.3-four+4").expect("test data must be valid semver");
        let next = build(&input, "7").expect("we expected build() to work");
        assert_eq!(next.to_string(), "1.2.3-four+7");
    }

    #[test]
    fn semver_spec_compliance() {
        // Test that major/minor/patch bumps remove prerelease and build metadata
        let input = Version::parse("1.2.3-alpha.1+build.123").expect("valid semver");
        let major_bump = major(&input);
        assert_eq!(major_bump.to_string(), "2.0.0");
        assert!(major_bump.pre.is_empty());
        assert!(major_bump.build.is_empty());

        let minor_bump = minor(&input);
        assert_eq!(minor_bump.to_string(), "1.3.0");
        assert!(minor_bump.pre.is_empty());
        assert!(minor_bump.build.is_empty());

        let patch_bump = patch(&input);
        assert_eq!(patch_bump.to_string(), "1.2.4");
        assert!(patch_bump.pre.is_empty());
        assert!(patch_bump.build.is_empty());
    }

    #[test]
    fn prerelease_with_non_numeric_suffix() {
        // When prerelease ends with non-numeric, we should append .1
        let input = Version::parse("1.0.0-alpha.beta").expect("valid semver");
        let next = prerelease(&input, "").expect("should handle non-numeric suffix");
        assert_eq!(next.to_string(), "1.0.0-alpha.beta.1");

        // Complex prerelease with multiple dots
        let input = Version::parse("2.0.0-rc.1.alpha").expect("valid semver");
        let next = prerelease(&input, "").expect("should handle complex prerelease");
        assert_eq!(next.to_string(), "2.0.0-rc.1.alpha.1");
    }

    #[test]
    fn numeric_prerelease_handling() {
        // Pure numeric prerelease
        let input = Version::parse("1.0.0-0").expect("valid semver");
        let next = prerelease(&input, "").expect("should increment numeric");
        assert_eq!(next.to_string(), "1.0.0-1");

        // Numeric after identifier
        let input = Version::parse("1.0.0-alpha.0").expect("valid semver");
        let next = prerelease(&input, "").expect("should increment");
        assert_eq!(next.to_string(), "1.0.0-alpha.1");
    }

    #[test]
    fn zero_version_handling() {
        // 0.x.y versions should work normally
        let input = Version::parse("0.1.0").expect("valid semver");
        let minor_bump = minor(&input);
        assert_eq!(minor_bump.to_string(), "0.2.0");

        let input = Version::parse("0.0.1").expect("valid semver");
        let patch_bump = patch(&input);
        assert_eq!(patch_bump.to_string(), "0.0.2");

        // Major bump from 0.x.y should go to 1.0.0
        let input = Version::parse("0.5.3").expect("valid semver");
        let major_bump = major(&input);
        assert_eq!(major_bump.to_string(), "1.0.0");
    }

    #[test]
    fn build_metadata_preservation() {
        // Build metadata should be preserved when bumping prerelease
        let input = Version::parse("1.0.0-alpha+build.123").expect("valid semver");
        let next = prerelease(&input, "").expect("should preserve build");
        assert_eq!(next.to_string(), "1.0.0-alpha.1+build.123");

        // Build metadata should be updated when using build command
        let next = build(&input, "").expect("should update build");
        assert_eq!(next.to_string(), "1.0.0-alpha+build.124");
    }

    #[test]
    fn identifier_replacement() {
        // Replacing one prerelease identifier with another
        let input = Version::parse("1.0.0-alpha.5").expect("valid semver");
        let next = prerelease(&input, "beta").expect("should replace identifier");
        assert_eq!(next.to_string(), "1.0.0-beta.1");

        // Replacing with same identifier should increment
        let next = prerelease(&input, "alpha").expect("should increment same identifier");
        assert_eq!(next.to_string(), "1.0.0-alpha.6");
    }

    #[test]
    fn edge_case_separators() {
        // Testing both . and - as separators
        let input = Version::parse("1.0.0-rc-1").expect("valid semver with dash separator");
        let next = prerelease(&input, "").expect("should handle dash separator");
        assert_eq!(next.to_string(), "1.0.0-rc-2");

        let input = Version::parse("1.0.0-rc.1").expect("valid semver with dot separator");
        let next = prerelease(&input, "").expect("should handle dot separator");
        assert_eq!(next.to_string(), "1.0.0-rc.2");

        // Mixed separators in identifier
        let input = Version::parse("1.0.0-alpha-beta.1").expect("valid semver");
        let next = prerelease(&input, "").expect("should handle mixed separators");
        assert_eq!(next.to_string(), "1.0.0-alpha-beta.2");
    }
}
