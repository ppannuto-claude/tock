// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2026.

//! `code-tiers`: prototype tooling for Tock's Code Tiers annotation
//! system (see `doc/reference/trd-tiers.md` on tock/tock#4694), scoped
//! for now to a pilot on the `kernel` crate only.
//!
//! Two subcommands:
//! - `coverage`: how much of `kernel` is annotated yet.
//! - `suggest-start`: a small ranked list of low-risk leaf/near-leaf
//!   items to hand-annotate first.
//!
//! See README.md for scope limitations (name-based call resolution,
//! intra-crate only, minimal `#[cfg]` awareness).

mod callgraph;
mod item_index;
mod module_tree;
mod report_coverage;
mod report_suggest_start;
mod tier;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "code-tiers",
    about = "Tock Code Tiers annotation pilot tooling"
)]
struct Cli {
    /// Path to the Tock repo root (containing `kernel/`). Defaults to the
    /// current directory.
    #[arg(long, global = true, default_value = ".")]
    repo_root: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Json,
    Markdown,
}

#[derive(Subcommand)]
enum Command {
    /// Report how much of the kernel crate is annotated with Code Tiers.
    Coverage {
        #[arg(long, value_enum, default_value = "json")]
        format: Format,
    },
    /// Suggest a small, low-risk leaf/near-leaf item to start annotating.
    SuggestStart {
        #[arg(long, value_enum, default_value = "markdown")]
        format: Format,
        #[arg(long, default_value_t = 10)]
        top: usize,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let repo_root = cli
        .repo_root
        .canonicalize()
        .with_context(|| format!("resolving repo root {}", cli.repo_root.display()))?;
    let kernel_src = repo_root.join("kernel").join("src");

    let index =
        item_index::index_crate(&kernel_src, &repo_root).context("indexing kernel crate")?;

    for warning in &index.warnings {
        eprintln!("warning: {warning}");
    }
    eprintln!(
        "indexed {} files, {} items, {} function/method bodies",
        index.files.len(),
        index.items.len(),
        index.bodies.len()
    );

    match cli.command {
        Command::Coverage { format } => {
            let report = report_coverage::build_report("kernel", &index.items);
            match format {
                Format::Json => println!("{}", serde_json::to_string_pretty(&report)?),
                Format::Markdown => println!("{}", report_coverage::render_markdown(&report)),
            }
        }
        Command::SuggestStart { format, top } => {
            let graph = callgraph::build_graph(&index.items, &index.bodies);
            let candidates =
                report_suggest_start::rank_candidates(&index.items, &index.bodies, &graph, top);
            match format {
                Format::Json => println!("{}", serde_json::to_string_pretty(&candidates)?),
                Format::Markdown => {
                    println!(
                        "{}",
                        report_suggest_start::render_markdown("kernel", &candidates)
                    )
                }
            }
        }
    }

    Ok(())
}
