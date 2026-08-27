// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2026.

//! Walks `kernel/src`'s module tree with `syn`, producing a flat list of
//! every doc-comment-eligible item along with its parsed Code Tier
//! annotation (if any).
//!
//! Scope limitations (see README.md): this is a syntactic pass only, no
//! type resolution. `#[cfg(...)]`-gated modules (the only real-world case
//! found in `kernel/src` today being inline `#[cfg(test)]` submodules) are
//! skipped entirely -- they're not production code subject to the tiers
//! campaign, and this pilot does not otherwise attempt to evaluate cfg
//! predicates. Impl-block methods take their own `syn` visibility, which
//! for trait-impl methods is typically `Inherited` even when the trait
//! itself is public; this under-counts such methods as "private" for the
//! public-API coverage number -- a known, documented simplification.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use syn::{ImplItem, Item, TraitItem};

use crate::module_tree::resolve_child_module_file;
use crate::tier::{Tier, parse_code_tier};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Fn,
    Method,
    Struct,
    Enum,
    Trait,
    Impl,
    Const,
    Static,
    TypeAlias,
    Mod,
}

impl ItemKind {
    pub fn is_callable(self) -> bool {
        matches!(self, ItemKind::Fn | ItemKind::Method)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Public,
    /// `pub(crate)`, `pub(super)`, `pub(in ...)` -- all collapsed to one
    /// bucket for this prototype; the distinction doesn't matter for a
    /// single-crate coverage report.
    Crate,
    Private,
}

fn map_visibility(vis: &syn::Visibility) -> Visibility {
    match vis {
        syn::Visibility::Public(_) => Visibility::Public,
        syn::Visibility::Restricted(_) => Visibility::Crate,
        syn::Visibility::Inherited => Visibility::Private,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemRecord {
    pub qualified_path: String,
    pub file: String,
    pub line: usize,
    pub kind: ItemKind,
    pub visibility: Visibility,
    pub annotated: bool,
    pub tier: Option<Tier>,
}

/// A function/method body, kept separately from `ItemRecord` (which is
/// the serializable report data) since `syn::Block` isn't meant to be
/// serialized -- only used transiently by the call-graph pass.
pub struct FnBody {
    pub qualified_path: String,
    pub block: syn::Block,
}

pub struct IndexResult {
    pub items: Vec<ItemRecord>,
    pub bodies: Vec<FnBody>,
    pub files: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

struct Ctx<'a> {
    repo_root: &'a Path,
    items: Vec<ItemRecord>,
    bodies: Vec<FnBody>,
    files: Vec<PathBuf>,
    warnings: Vec<String>,
}

/// Indexes the `kernel` crate starting from `kernel_src_root/lib.rs`.
pub fn index_crate(kernel_src_root: &Path, repo_root: &Path) -> Result<IndexResult> {
    let lib_rs = kernel_src_root.join("lib.rs");
    let mut ctx = Ctx {
        repo_root,
        items: Vec::new(),
        bodies: Vec::new(),
        files: Vec::new(),
        warnings: Vec::new(),
    };
    walk_file(&lib_rs, kernel_src_root, &["kernel".to_string()], &mut ctx)?;
    Ok(IndexResult {
        items: ctx.items,
        bodies: ctx.bodies,
        files: ctx.files,
        warnings: ctx.warnings,
    })
}

fn has_cfg(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("cfg"))
}

fn extract_doc(attrs: &[syn::Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let syn::Meta::NameValue(nv) = &attr.meta {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) = &nv.value
                {
                    lines.push(s.value());
                }
            }
        }
    }
    lines.join("\n")
}

fn qualify(module_path: &[String], name: &str) -> String {
    if module_path.is_empty() {
        name.to_string()
    } else {
        format!("{}::{}", module_path.join("::"), name)
    }
}

fn line_of(span: proc_macro2::Span) -> usize {
    span.start().line
}

fn push_item(
    ctx: &mut Ctx,
    module_path: &[String],
    name: &str,
    kind: ItemKind,
    vis: &syn::Visibility,
    attrs: &[syn::Attribute],
    file_rel: &Path,
    line: usize,
    body: Option<&syn::Block>,
) {
    let qualified_path = qualify(module_path, name);
    let doc = extract_doc(attrs);
    let tier = parse_code_tier(&doc);
    if let Some(block) = body {
        ctx.bodies.push(FnBody {
            qualified_path: qualified_path.clone(),
            block: block.clone(),
        });
    }
    ctx.items.push(ItemRecord {
        qualified_path,
        file: file_rel.to_string_lossy().into_owned(),
        line,
        kind,
        visibility: map_visibility(vis),
        annotated: tier.is_some(),
        tier,
    });
}

fn walk_file(file: &Path, dir: &Path, module_path: &[String], ctx: &mut Ctx) -> Result<()> {
    let src = fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let parsed = syn::parse_file(&src).with_context(|| format!("parsing {}", file.display()))?;
    let rel = file
        .strip_prefix(ctx.repo_root)
        .unwrap_or(file)
        .to_path_buf();
    ctx.files.push(rel.clone());
    walk_items(&parsed.items, dir, module_path, &rel, ctx);
    Ok(())
}

fn walk_items(items: &[Item], dir: &Path, module_path: &[String], file_rel: &Path, ctx: &mut Ctx) {
    for item in items {
        match item {
            Item::Fn(f) => push_item(
                ctx,
                module_path,
                &f.sig.ident.to_string(),
                ItemKind::Fn,
                &f.vis,
                &f.attrs,
                file_rel,
                line_of(f.sig.ident.span()),
                Some(&f.block),
            ),
            Item::Struct(s) => push_item(
                ctx,
                module_path,
                &s.ident.to_string(),
                ItemKind::Struct,
                &s.vis,
                &s.attrs,
                file_rel,
                line_of(s.ident.span()),
                None,
            ),
            Item::Enum(e) => push_item(
                ctx,
                module_path,
                &e.ident.to_string(),
                ItemKind::Enum,
                &e.vis,
                &e.attrs,
                file_rel,
                line_of(e.ident.span()),
                None,
            ),
            Item::Const(c) => push_item(
                ctx,
                module_path,
                &c.ident.to_string(),
                ItemKind::Const,
                &c.vis,
                &c.attrs,
                file_rel,
                line_of(c.ident.span()),
                None,
            ),
            Item::Static(s) => push_item(
                ctx,
                module_path,
                &s.ident.to_string(),
                ItemKind::Static,
                &s.vis,
                &s.attrs,
                file_rel,
                line_of(s.ident.span()),
                None,
            ),
            Item::Type(t) => push_item(
                ctx,
                module_path,
                &t.ident.to_string(),
                ItemKind::TypeAlias,
                &t.vis,
                &t.attrs,
                file_rel,
                line_of(t.ident.span()),
                None,
            ),
            Item::Trait(t) => {
                push_item(
                    ctx,
                    module_path,
                    &t.ident.to_string(),
                    ItemKind::Trait,
                    &t.vis,
                    &t.attrs,
                    file_rel,
                    line_of(t.ident.span()),
                    None,
                );
                let mut trait_path = module_path.to_vec();
                trait_path.push(t.ident.to_string());
                for ti in &t.items {
                    if let TraitItem::Fn(m) = ti {
                        push_item(
                            ctx,
                            &trait_path,
                            &m.sig.ident.to_string(),
                            ItemKind::Method,
                            &t.vis,
                            &m.attrs,
                            file_rel,
                            line_of(m.sig.ident.span()),
                            m.default.as_ref(),
                        );
                    }
                }
            }
            Item::Impl(imp) => {
                let self_ty = type_to_string(&imp.self_ty);
                let mut impl_path = module_path.to_vec();
                impl_path.push(self_ty);
                // `impl` blocks have no `vis` field of their own (they
                // aren't separately importable items); use `Inherited`
                // as a documented stand-in when reporting visibility.
                push_item(
                    ctx,
                    module_path,
                    impl_path.last().unwrap(),
                    ItemKind::Impl,
                    &syn::Visibility::Inherited,
                    &imp.attrs,
                    file_rel,
                    line_of(imp.impl_token.span),
                    None,
                );
                for ii in &imp.items {
                    if let ImplItem::Fn(m) = ii {
                        push_item(
                            ctx,
                            &impl_path,
                            &m.sig.ident.to_string(),
                            ItemKind::Method,
                            &m.vis,
                            &m.attrs,
                            file_rel,
                            line_of(m.sig.ident.span()),
                            Some(&m.block),
                        );
                    }
                }
            }
            Item::Mod(m) => {
                if has_cfg(&m.attrs) {
                    // Out of scope for v1: cfg-gated modules (in practice,
                    // today, only inline `#[cfg(test)]`/`#[cfg(feature =
                    // "flux")]` submodules) are skipped entirely rather
                    // than evaluated.
                    continue;
                }
                push_item(
                    ctx,
                    module_path,
                    &m.ident.to_string(),
                    ItemKind::Mod,
                    &m.vis,
                    &m.attrs,
                    file_rel,
                    line_of(m.ident.span()),
                    None,
                );
                let mut child_path = module_path.to_vec();
                child_path.push(m.ident.to_string());
                match &m.content {
                    Some((_, inline_items)) => {
                        let child_dir = dir.join(m.ident.to_string());
                        walk_items(inline_items, &child_dir, &child_path, file_rel, ctx);
                    }
                    None => match resolve_child_module_file(dir, &m.ident.to_string()) {
                        Some((child_file, child_dir)) => {
                            if let Err(e) = walk_file(&child_file, &child_dir, &child_path, ctx) {
                                ctx.warnings.push(format!("{e:#}"));
                            }
                        }
                        None => ctx.warnings.push(format!(
                            "could not resolve file for `mod {}` declared in {}",
                            m.ident,
                            file_rel.display()
                        )),
                    },
                }
            }
            _ => {}
        }
    }
}

fn type_to_string(ty: &syn::Type) -> String {
    // `quote!`'s token-stream Display puts a space between every token
    // (e.g. `GrantData < 'a , T >`); tidy up the common generic-brackets
    // case so paths read naturally in reports. Not a general Rust
    // pretty-printer -- just enough for the path/generic shapes that
    // show up as `impl` self-types in this crate.
    let raw = quote::quote!(#ty).to_string();
    raw.replace(" ::", "::")
        .replace(":: ", "::")
        .replace(" <", "<")
        .replace("< ", "<")
        .replace(" >", ">")
        .replace(" ,", ",")
}
