// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2026.

//! Renders the annotation-coverage report (the `coverage` subcommand):
//! how much of the kernel crate has an explicit `# Code Tier` annotation
//! yet, broken out as a machine-readable JSON report (the source of
//! truth, diffable over time to track campaign progress) and a
//! human-readable Markdown worklist (paste-ready for a tracking issue).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Serialize;

use crate::item_index::{ItemKind, ItemRecord, Visibility};
use crate::tier::{Assurance, Importance};

#[derive(Serialize)]
pub struct Totals {
    pub items_total: usize,
    pub items_annotated: usize,
    pub public_items_total: usize,
    pub public_items_annotated: usize,
}

#[derive(Serialize)]
pub struct TierHistogram {
    pub assurance: BTreeMap<String, usize>,
    pub importance: BTreeMap<String, usize>,
}

#[derive(Serialize)]
pub struct CoverageReport<'a> {
    pub crate_name: &'a str,
    pub totals: Totals,
    pub tier_histogram: TierHistogram,
    pub items: &'a [ItemRecord],
}

pub fn build_report<'a>(crate_name: &'a str, items: &'a [ItemRecord]) -> CoverageReport<'a> {
    let items_total = items.len();
    let items_annotated = items.iter().filter(|i| i.annotated).count();
    let public_items: Vec<&ItemRecord> = items
        .iter()
        .filter(|i| i.visibility == Visibility::Public)
        .collect();
    let public_items_total = public_items.len();
    let public_items_annotated = public_items.iter().filter(|i| i.annotated).count();

    let mut assurance: BTreeMap<String, usize> = BTreeMap::new();
    let mut importance: BTreeMap<String, usize> = BTreeMap::new();
    for name in [
        "FormallyVerified",
        "ExtensivelyTested",
        "FunctionallyTested",
        "Normal",
    ] {
        assurance.insert(name.to_string(), 0);
    }
    for name in ["Critical", "WidelyUsed", "Normal", "Experimental"] {
        importance.insert(name.to_string(), 0);
    }
    assurance.insert("unannotated".to_string(), 0);
    importance.insert("unannotated".to_string(), 0);

    for item in items {
        match &item.tier {
            Some(t) => {
                *assurance.entry(assurance_key(t.assurance)).or_insert(0) += 1;
                *importance.entry(importance_key(t.importance)).or_insert(0) += 1;
            }
            None => {
                *assurance.entry("unannotated".to_string()).or_insert(0) += 1;
                *importance.entry("unannotated".to_string()).or_insert(0) += 1;
            }
        }
    }

    CoverageReport {
        crate_name,
        totals: Totals {
            items_total,
            items_annotated,
            public_items_total,
            public_items_annotated,
        },
        tier_histogram: TierHistogram {
            assurance,
            importance,
        },
        items,
    }
}

fn assurance_key(a: Assurance) -> String {
    match a {
        Assurance::FormallyVerified => "FormallyVerified",
        Assurance::ExtensivelyTested => "ExtensivelyTested",
        Assurance::FunctionallyTested => "FunctionallyTested",
        Assurance::Normal => "Normal",
    }
    .to_string()
}

fn importance_key(i: Importance) -> String {
    match i {
        Importance::Critical => "Critical",
        Importance::WidelyUsed => "WidelyUsed",
        Importance::Normal => "Normal",
        Importance::Experimental => "Experimental",
    }
    .to_string()
}

fn kind_label(k: ItemKind) -> &'static str {
    match k {
        ItemKind::Fn => "fn",
        ItemKind::Method => "method",
        ItemKind::Struct => "struct",
        ItemKind::Enum => "enum",
        ItemKind::Trait => "trait",
        ItemKind::Impl => "impl",
        ItemKind::Const => "const",
        ItemKind::Static => "static",
        ItemKind::TypeAlias => "type",
        ItemKind::Mod => "mod",
    }
}

/// Renders a per-file Markdown checklist of unannotated **public** items
/// -- the direct worklist for the annotation campaign.
pub fn render_markdown(report: &CoverageReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Code Tier coverage: `{}`", report.crate_name);
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Public API surface: {}/{} annotated. All items: {}/{} annotated.",
        report.totals.public_items_annotated,
        report.totals.public_items_total,
        report.totals.items_annotated,
        report.totals.items_total,
    );
    let _ = writeln!(out);

    let mut by_file: BTreeMap<&str, Vec<&ItemRecord>> = BTreeMap::new();
    for item in report.items {
        if item.visibility == Visibility::Public && !item.annotated {
            by_file.entry(&item.file).or_default().push(item);
        }
    }

    for (file, mut items) in by_file {
        items.sort_by_key(|i| i.line);
        let _ = writeln!(out, "## {file}");
        let _ = writeln!(out);
        for item in items {
            let _ = writeln!(
                out,
                "- [ ] `{}` {} (line {})",
                kind_label(item.kind),
                item.qualified_path,
                item.line
            );
        }
        let _ = writeln!(out);
    }

    out
}
