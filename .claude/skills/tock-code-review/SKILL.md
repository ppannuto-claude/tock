---
name: tock-code-review
description: Heavy-weight, Tock-specific review of a change set (working tree, branch, or PR) against this repository's documented conventions and its recurring defect classes - unsafe and capability rules, callbacks issued from downcalls, buffers lost from TakeCell on error paths, MMIO register and interrupt handling, HIL ripple effects, SyscallDriver rules, and board/component structure. Use at milestones - when a substantial piece of work is finished, before opening or updating a pull request, or when explicitly asked to review Tock code - not after routine edits.
---

# Tock Code Review

A deliberate, evidence-based review pass over a change set in the Tock kernel
repository. It is slower than a lint run on purpose: most of the defects that
matter here are invisible to rustc and clippy, because they are violations of
Tock's execution model (no allocation, no unwinding, interrupt-driven
callbacks, static resource ownership) rather than of Rust's type system.

## When to run this

- A coding agent has finished a milestone worth reviewing as a unit: a new
  capsule, a chip peripheral, a board, a refactor that touched several crates.
- A change is about to be submitted upstream, or an open pull request is about
  to be updated.
- Someone asks for a review of a branch, PR, or diff.

Do not run it after every edit. It reads surrounding code, builds boards, and
produces a written report; that cost is only worth paying against a body of
work that is complete enough to judge.

## What it is not

It does not replace `make prepush` - it *runs* it, then spends its effort on
what the tooling cannot see. It does not rewrite the code by default: report
findings and let the author decide, unless asked to fix them.

## Procedure

### 1. Establish scope

Identify exactly what is under review, and read all of it:

```sh
git fetch origin
git diff --stat $(git merge-base HEAD origin/master)..HEAD    # branch
git diff --stat                                               # working tree
gh pr diff <N>                                                # pull request
```

Then group the changed files by subsystem (`kernel/`, `kernel/src/hil/`,
`capsules/`, `chips/`, `boards/`, `arch/`, `libraries/`, `tools/`, `doc/`).
The review criteria genuinely differ per subsystem - see
`references/subsystem-criteria.md`.

Read the surrounding code, not just the diff hunks. Most of the defect classes
in `references/bug-patterns.md` are invisible in a hunk: whether a buffer is
replaced on *every* return path, or whether a callback path is reachable from
a downcall, is a property of the whole function or the whole capsule.

Note the change's weight class while you are here. `doc/CodeReview.md` splits
pull requests into "upkeep" and "significant"; significant changes (new
traits, kernel components, new modules, build system changes) require review
by the whole core team, and a change that crosses that line is worth flagging
to the author early.

### 2. Run the mechanical checks first

So that review attention is not spent on what a tool would have caught, and so
findings are not reported against code that does not build. See
`references/mechanical-checks.md` for the commands, which boards to build for
a given change, and how to reproduce a specific CI job locally.

### 3. Subsystem pass

Walk each changed subsystem against `references/subsystem-criteria.md`. This
folds in the rules from `AGENTS.md`, `doc/CodeReview.md`, `doc/CodeGoals.md`,
and `doc/Style.md`, plus the conventions that are only visible in the tree.

### 4. Defect-class pass

Walk the change against `references/bug-patterns.md`: the recurring Tock bug
classes, each with what it looks like and how to confirm it. This is where the
real findings usually come from.

### 5. Ripple check

Tock changes propagate along paths the compiler will happily let you ignore
when a crate is not in the default build. Check the ones that apply - the
recipes are in `references/subsystem-criteria.md` under "Ripple checks".
Changed HIL trait, changed capsule constructor, changed register struct,
changed `DRIVER_NUM`, and new kernel exports each have a fan-out to verify.

### 6. Verify every finding before reporting it

This is the step that decides whether the review is useful or noise.

- Confirm each finding against the actual code. Pattern-matching on a diff
  produces confident, wrong findings; open the file and follow the path.
- State a concrete failure scenario: the inputs, hardware state, or call
  sequence that reaches the bug, and what goes wrong. If you cannot construct
  one, it is a suggestion or a style note, not a bug - label it as such or
  drop it.
- Separate defects introduced by this change from pre-existing ones. Both are
  worth mentioning; conflating them wastes the author's time and misattributes
  blame.
- Prefer dropping an uncertain finding over padding the report. A review whose
  every item survives scrutiny gets acted on; one with three real findings
  buried in nine speculative ones does not.
- Where a claim depends on hardware behavior you cannot test, say so instead of
  asserting it.

### 7. Report

Order findings by severity, most severe first. For each:

- `path/to/file.rs:LINE` - the anchor.
- One sentence stating the defect.
- The failure scenario (from step 6).
- A suggested direction, only where you are confident in it.

Close with what you checked and, explicitly, what you could not: boards not
built, hardware not available, tests not run. A reviewer's silence is read as
"checked and fine", so make the gaps visible.

## Reporting constraints specific to this repository

- **Do not write prose for the human to post.** `AGENTS.md` and the AI policy
  in `.github/CONTRIBUTING.md` permit AI-assisted code but not AI-written text
  addressed to humans in issues, PR descriptions, or review comments. Deliver
  the review as technical findings for the author to act on and describe in
  their own words. Producing a polished, ready-to-paste review comment is the
  same violation with an extra step.
- Cite the rule you are applying (`AGENTS.md`, `doc/CodeReview.md`,
  `doc/Style.md`, a TRD in `doc/reference/`) when a finding rests on a written
  convention, so the author can check it rather than take it on faith.
