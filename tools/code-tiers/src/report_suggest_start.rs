// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2026.

//! Renders the `suggest-start` report: a small ranked list of low-risk
//! leaf/near-leaf items to hand-annotate first, to bootstrap the
//! annotation campaign rather than starting from a random or
//! high-centrality item. See `callgraph.rs` for the (name-based,
//! intra-crate-only) graph this ranking is built from.

use std::fmt::Write as _;

use serde::Serialize;

use crate::callgraph::Graph;
use crate::item_index::{FnBody, ItemRecord};

#[derive(Serialize)]
pub struct Candidate {
    pub qualified_path: String,
    pub file: String,
    pub line: usize,
    pub out_degree: usize,
    pub in_degree: usize,
    pub body_lines: usize,
    pub rationale: String,
}

fn block_line_count(block: &syn::Block) -> usize {
    let span = block.brace_token.span.join();
    let start = span.start().line;
    let end = span.end().line;
    end.saturating_sub(start) + 1
}

/// Ranks unannotated callable items by (out_degree, in_degree, size),
/// preferring true leaves (`out_degree == 0`) and widening to
/// `out_degree <= 1` ("near-leaf") only if there are too few true
/// leaves to choose from.
pub fn rank_candidates(
    items: &[ItemRecord],
    bodies: &[FnBody],
    graph: &Graph,
    top_n: usize,
) -> Vec<Candidate> {
    // Only consider items that actually have a body: an abstract trait
    // method signature (no default impl) trivially has 0 outgoing calls
    // and 0 body lines, which would otherwise dominate the ranking
    // without being a meaningful "small method" to hand-annotate first.
    let has_body: std::collections::HashSet<&str> =
        bodies.iter().map(|b| b.qualified_path.as_str()).collect();
    let pool: Vec<&ItemRecord> = items
        .iter()
        .filter(|i| i.kind.is_callable() && !i.annotated)
        .filter(|i| has_body.contains(i.qualified_path.as_str()))
        .collect();

    let true_leaves: Vec<&&ItemRecord> = pool
        .iter()
        .filter(|i| *graph.out_degree.get(&i.qualified_path).unwrap_or(&0) == 0)
        .collect();

    let source: Vec<&ItemRecord> = if true_leaves.len() >= top_n.max(1) {
        true_leaves.into_iter().copied().collect()
    } else {
        pool.iter()
            .filter(|i| *graph.out_degree.get(&i.qualified_path).unwrap_or(&0) <= 1)
            .copied()
            .collect()
    };

    let mut scored: Vec<(usize, usize, usize, &ItemRecord)> = source
        .into_iter()
        .map(|item| {
            let out_d = *graph.out_degree.get(&item.qualified_path).unwrap_or(&0);
            let in_d = *graph.in_degree.get(&item.qualified_path).unwrap_or(&0);
            let lines = bodies
                .iter()
                .find(|b| b.qualified_path == item.qualified_path)
                .map(|b| block_line_count(&b.block))
                .unwrap_or(0);
            (out_d, in_d, lines, item)
        })
        .collect();

    scored.sort_by(|a, b| (a.0, a.1, a.2).cmp(&(b.0, b.1, b.2)));

    scored
        .into_iter()
        .take(top_n)
        .map(|(out_d, in_d, lines, item)| Candidate {
            qualified_path: item.qualified_path.clone(),
            file: item.file.clone(),
            line: item.line,
            out_degree: out_d,
            in_degree: in_d,
            body_lines: lines,
            rationale: format!(
                "{out_d} outgoing intra-crate call(s), {in_d} incoming intra-crate call(s), {lines} body line(s)"
            ),
        })
        .collect()
}

pub fn render_markdown(crate_name: &str, candidates: &[Candidate]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Code Tier annotation starting points: `{crate_name}`"
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Ranked leaf/near-leaf candidates (intra-crate call graph, name-based -- see README for precision caveats):"
    );
    let _ = writeln!(out);
    for (i, c) in candidates.iter().enumerate() {
        let _ = writeln!(
            out,
            "{}. `{}` ({}:{}) -- {}",
            i + 1,
            c.qualified_path,
            c.file,
            c.line,
            c.rationale
        );
    }
    if let Some(top) = candidates.first() {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "**Recommended starting item:** `{}` -- {}",
            top.qualified_path, top.rationale
        );
    }
    out
}
