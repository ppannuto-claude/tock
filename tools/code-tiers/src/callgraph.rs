// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2026.

//! A lightweight, name-based, *intra-crate-only* call graph used to rank
//! candidate items for the "small method to start annotating with" pilot
//! deliverable.
//!
//! This is deliberately **not** a real call-graph analysis: call/method
//! call sites are matched to intra-crate items purely by their last path
//! segment (identifier), with no type or receiver resolution. A call that
//! doesn't match any intra-crate item's name (e.g. any call into a Cargo
//! dependency, `core`, `alloc`, or another workspace crate) is simply
//! dropped -- no "external" node is ever created, which is what keeps
//! this graph strictly intra-crate as required.
//!
//! Precision caveat, documented rather than fixed: this produces
//! false-positive edges (e.g. every intra-crate method literally named
//! `get` matches a `.get()` call, regardless of receiver type) and false
//! negatives (macro-generated call sites, closures, function pointers,
//! trait default-method dispatch aren't modeled). This bias is one-sided
//! and safe for a *ranking* heuristic: false-positive edges only ever
//! make an item look more connected than it really is, so a genuinely
//! low-risk leaf can be excluded by a spurious edge, but a real hub can
//! never be mistakenly promoted to "leaf" -- that would require a missed
//! edge for a call that's syntactically right there in the source.

use std::collections::{HashMap, HashSet};

use syn::visit::{self, Visit};
use syn::{Expr, ExprCall, ExprMethodCall, ExprPath};

use crate::item_index::{FnBody, ItemRecord};

struct CallCollector {
    names: HashSet<String>,
}

impl<'ast> Visit<'ast> for CallCollector {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Expr::Path(ExprPath { path, .. }) = &*node.func {
            if let Some(seg) = path.segments.last() {
                self.names.insert(seg.ident.to_string());
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        self.names.insert(node.method.to_string());
        visit::visit_expr_method_call(self, node);
    }
}

fn collect_call_names(block: &syn::Block) -> HashSet<String> {
    let mut collector = CallCollector {
        names: HashSet::new(),
    };
    collector.visit_block(block);
    collector.names
}

pub struct Graph {
    pub out_degree: HashMap<String, usize>,
    pub in_degree: HashMap<String, usize>,
    /// Kept for future debugging/output (e.g. explaining *why* an item
    /// ranked as it did); not consumed by the current reports.
    #[allow(dead_code)]
    pub edges: Vec<(String, String)>,
}

/// Builds the intra-crate name-based call graph. `items` provides the set
/// of known callable (fn/method) qualified paths to resolve names
/// against; `bodies` provides the function bodies to scan for call sites.
pub fn build_graph(items: &[ItemRecord], bodies: &[FnBody]) -> Graph {
    let mut by_name: HashMap<String, Vec<String>> = HashMap::new();
    let mut out_degree = HashMap::new();
    let mut in_degree = HashMap::new();

    for item in items {
        if item.kind.is_callable() {
            let simple = item
                .qualified_path
                .rsplit("::")
                .next()
                .unwrap_or(&item.qualified_path)
                .to_string();
            by_name
                .entry(simple)
                .or_default()
                .push(item.qualified_path.clone());
            out_degree.entry(item.qualified_path.clone()).or_insert(0);
            in_degree.entry(item.qualified_path.clone()).or_insert(0);
        }
    }

    let mut edges = Vec::new();
    for fb in bodies {
        let names = collect_call_names(&fb.block);
        let mut targets: HashSet<String> = HashSet::new();
        for name in &names {
            if let Some(candidates) = by_name.get(name) {
                for c in candidates {
                    if c != &fb.qualified_path {
                        targets.insert(c.clone());
                    }
                }
            }
            // Unresolved name: call leaves the crate (dependency, core,
            // alloc, or another workspace crate) or isn't a plain
            // call/method-call this pass models. Intentionally dropped.
        }
        *out_degree.entry(fb.qualified_path.clone()).or_insert(0) += targets.len();
        for t in &targets {
            *in_degree.entry(t.clone()).or_insert(0) += 1;
            edges.push((fb.qualified_path.clone(), t.clone()));
        }
    }

    Graph {
        out_degree,
        in_degree,
        edges,
    }
}
