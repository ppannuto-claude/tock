// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2026.

//! Resolves Rust `mod` declarations to the source files that back them.
//!
//! This is a small, purpose-built resolver (not full cargo/rustc module
//! resolution) that only needs to handle the two forms `kernel/src` uses:
//! `mod name;` backed by `name.rs`, or by `name/mod.rs`. Either way, any
//! further `mod` declarations *inside* `name` resolve relative to a
//! directory named `name` alongside wherever `name` itself was found.

use std::path::{Path, PathBuf};

/// Given the directory a module's children live in (`dir`) and a child
/// module's identifier `name`, resolve which source file backs that
/// child module and the directory *its own* children would live in.
pub fn resolve_child_module_file(dir: &Path, name: &str) -> Option<(PathBuf, PathBuf)> {
    let flat = dir.join(format!("{name}.rs"));
    let nested = dir.join(name).join("mod.rs");
    let child_dir = dir.join(name);
    if flat.is_file() {
        Some((flat, child_dir))
    } else if nested.is_file() {
        Some((nested, child_dir))
    } else {
        None
    }
}
