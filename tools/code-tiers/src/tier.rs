// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2026.

//! Parses the `# Code Tier` doc-comment section defined by the draft TRD
//! (tock/tock#4694, `doc/reference/trd-tiers.md`):
//!
//! ```text
//! # Code Tier
//!
//! - Assurance: <tier>
//! - Importance: <tier>
//! ```

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Assurance {
    FormallyVerified,
    ExtensivelyTested,
    FunctionallyTested,
    Normal,
}

impl Assurance {
    fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "Formally Verified" => Some(Self::FormallyVerified),
            "Extensively Tested" => Some(Self::ExtensivelyTested),
            "Functionally Tested" => Some(Self::FunctionallyTested),
            "Normal" => Some(Self::Normal),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Importance {
    Critical,
    WidelyUsed,
    Normal,
    Experimental,
}

impl Importance {
    fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "Critical" => Some(Self::Critical),
            "Widely Used" => Some(Self::WidelyUsed),
            "Normal" => Some(Self::Normal),
            "Experimental" => Some(Self::Experimental),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Tier {
    pub assurance: Assurance,
    pub importance: Importance,
}

/// Parses a `# Code Tier` section out of a joined doc-comment string.
/// Returns `None` if no such section is present, or if it's present but
/// missing either dimension (an item is only ever counted as annotated
/// when both dimensions parse successfully).
pub fn parse_code_tier(doc: &str) -> Option<Tier> {
    let mut lines = doc.lines();
    lines.by_ref().find(|l| l.trim() == "# Code Tier")?;

    let mut assurance = None;
    let mut importance = None;
    for line in lines {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(rest) = t.strip_prefix("- Assurance:") {
            assurance = Assurance::parse(rest);
        } else if let Some(rest) = t.strip_prefix("- Importance:") {
            importance = Importance::parse(rest);
        } else if t.starts_with('#') {
            // Reached the next doc-comment section; stop scanning.
            break;
        }
        // Otherwise: a free-form description line under the header, ignored.
    }

    match (assurance, importance) {
        (Some(assurance), Some(importance)) => Some(Tier {
            assurance,
            importance,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_formed_section() {
        // Note: uses Assurance/Importance values the other way round from
        // the literal example in the draft TRD (tock/tock#4694), whose own
        // worked example ("Assurance: Critical") doesn't actually match a
        // value from its own Assurance tier table -- "Critical" is an
        // Importance tier. Using internally-consistent values here instead.
        let doc = "Create a new Tock process.\n\n# Code Tier\n\n- Assurance: Extensively Tested\n- Importance: Critical\n";
        let tier = parse_code_tier(doc).expect("should parse");
        assert_eq!(tier.assurance, Assurance::ExtensivelyTested);
        assert_eq!(tier.importance, Importance::Critical);
    }

    #[test]
    fn missing_section_is_none() {
        assert!(parse_code_tier("just a normal doc comment").is_none());
    }

    #[test]
    fn incomplete_section_is_none() {
        let doc = "# Code Tier\n\n- Assurance: Critical\n";
        assert!(parse_code_tier(doc).is_none());
    }
}
