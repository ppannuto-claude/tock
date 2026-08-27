# code-tiers

Prototype tooling for Tock's "Code Tiers" annotation system (draft TRD,
[tock/tock#4694](https://github.com/tock/tock/pull/4694),
`doc/reference/trd-tiers.md` on that PR branch). Scoped, for now, to a pilot
on the `kernel` crate only, with the goal of getting `kernel` fully
annotated before considering the rest of the workspace.

Code Tiers marks items with two independent dimensions in a doc-comment
section:

```rust
/// # Code Tier
///
/// - Assurance: Extensively Tested
/// - Importance: Critical
pub fn some_function();
```

- **Assurance**: `Formally Verified` > `Extensively Tested` >
  `Functionally Tested` > `Normal`
- **Importance**: `Critical` > `Widely Used` > `Normal` > `Experimental`

## Usage

From the repo root:

```sh
cargo run -p code-tiers -- coverage --format markdown
cargo run -p code-tiers -- suggest-start --format markdown --top 10
```

Pass `--repo-root <path>` if not running from the Tock repo root.

- `coverage`: reports how much of `kernel` has an explicit `# Code Tier`
  section yet -- totals, a tier histogram, and (in `--format markdown`) a
  per-file checklist of unannotated **public** items, paste-ready for a
  tracking issue. `--format json` (the default) is the machine-readable
  source of truth, meant to be diffed over time to track campaign
  progress.
- `suggest-start`: ranks unannotated functions/methods by how
  self-contained they are in the crate's own call graph, and recommends
  one as a low-risk starting point for the annotation campaign -- a leaf
  or near-leaf, not a random or high-centrality item.

## Scope and known limitations

This is a syntactic (`syn`-based) tool with **no type resolution and no
rustc internals** -- it does not compile the crate, does not resolve
generics or trait dispatch, and does not follow `#[cfg]` feature
predicates. Specifically:

- **Call resolution is name-only.** `suggest-start`'s call graph matches
  `foo()` / `x.method()` call sites to intra-crate items purely by their
  last identifier segment, with no receiver-type or generic resolution.
  This produces false-positive edges (every intra-crate item literally
  named `get` matches every `.get()` call, regardless of receiver type)
  and false negatives (macro-generated call sites, closures, function
  pointers, and trait default-method dispatch aren't modeled). This bias
  is deliberately one-sided and safe for a *ranking* heuristic: a
  false-positive edge can only make an item look more connected than it
  is, so a genuine leaf might occasionally be excluded by a spurious
  edge, but a real hub can never be mistakenly promoted to "leaf"
  status -- that would require a missed edge for a call that's
  syntactically right there in the source.
- **Strictly intra-crate.** A call/method-call name that doesn't match
  any item in `kernel`'s own index (a Cargo dependency, `core`, `alloc`,
  or another Tock workspace crate) is simply dropped -- it never creates
  an "external" graph node, is never counted toward degree, and is never
  flagged as anything. This is intentional scope, not a bug: this pilot
  never reasons about code outside `kernel`.
- **`#[cfg(...)]`-gated modules are skipped entirely**, not evaluated.
  Today in `kernel/src` this only affects inline `#[cfg(test)]` (and one
  `#[cfg(feature = "flux")]`) submodules; if `kernel/src` later gains a
  `#[cfg]`-gated *top-level* module, this tool will silently include or
  exclude it incorrectly depending on which feature/config it happens to
  be built without -- this is unvalidated for that case.
- **"Unannotated" means no `# Code Tier` section was found**, full stop
  -- even though the TRD itself defines the *default* as Assurance:
  Normal / Importance: Normal when no annotation is present. For the
  purposes of this pilot's "fully annotated" goal, an unannotated item is
  treated as a real gap requiring an explicit human decision (even if
  that decision turns out to be "yes, Normal/Normal is correct"), not as
  a silently-fine default -- mirroring how Tock's existing (unenforced)
  `SAFETY:` comment convention already expects explicit presence rather
  than treating silence as acceptable.
- **`impl`-block method visibility is approximate.** Methods take their
  own `syn` visibility, which for methods in a `impl Trait for Type`
  block is typically `Inherited` (private) even when the trait itself is
  public. This under-counts some genuinely-public trait-impl methods in
  the "public API surface" coverage numbers -- a known simplification,
  not fixed in this prototype.
- **Not wired into CI.** This is a local/manual tool for now: there's no
  annotation baseline yet to gate on, and the precision caveats above
  make it unsuitable as a hard gate before it's been used for a while.
  It's a normal `tools/` Cargo workspace member, so adding a CI job
  later (e.g. "coverage must not regress") is a small follow-up, not a
  redesign.

## Relationship to the harder problem

The TRD's own open question (`doc/reference/trd-tiers.md`, Section 5) is
whether higher-tier code depending on lower-tier code should be an error
or a warning, and how that would be enforced. This tool doesn't attempt
that -- it only helps get `kernel` annotated in the first place. A real
violation checker needs actual call-graph edges (via rustc's MIR, in the
style of [Scrutinizer](https://github.com/brownsys/scrutinizer)/the
Sesame paper), not name-based syntactic matching, and is left as future
work. This tool's annotation parser (`tier.rs`) is written to be reusable
by that future tool unchanged.
