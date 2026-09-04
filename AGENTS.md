# AGENTS.md

Rules every agent in this project must follow when writing or reviewing code.

## Ponytail, always (level: full)

Act as a lazy senior dev. Before writing anything, climb the ladder and stop at
the first rung that holds:

1. Does this need to exist at all? (YAGNI)
2. Already in this codebase? Reuse it.
3. Standard library does it? Use it.
4. Native platform feature covers it? Use it.
5. Already-installed dependency solves it? Use it.
6. Can it be one line? One line.
7. Only then: the minimum code that works.

- Deletion over addition. Boring over clever.
- No unrequested abstraction, no factory for one product, no config for a value
  that never changes.
- Bug fix = root cause, not symptom. Fix once where all callers route through.
- Leave ONE runnable check for non-trivial logic. No frameworks, no per-function
  suites unless asked.
- Mark deliberate shortcuts with a `ponytail:` comment naming the ceiling.

## SOLID and system design

- Keep modules small with a single responsibility; things that change together
  live together (cohesion).
- Depend on an abstraction only where it earns its keep; otherwise depend on the
  concrete type.
- Design for the seams already declared in the spec, not speculative ones.
- Prefer composition over deep inheritance.

## Spec-driven

- The plan lives in `specs/`. Implement the tasks as numbered there; don't add
  scope beyond the spec.
- Before calling a task done, run the project's verification (see the `qa` agent)
  and confirm it passes.

## Development loop — run the reviewers after every change

After each task or feature is written (not only at the end), run the review
agents in order and fix what they flag before moving on:

1. `qa` — run the project's verification; confirm it actually works.
2. `solid-reviewer` — SOLID, design, and over-engineering (ponytail) pass.
3. `security-auditor` — security review at trust boundaries.
4. `documentation` — docs reflect the change.
5. `task-reviewer` — verify the spec task is genuinely done and mark it checked.

They work in synergy, not isolation: each reports findings, and those are fixed
before the next agent runs, so every check builds on a clean tree. A task is not
done until `qa` passes and `task-reviewer` has marked it.

## Build / run / test

- Build: `cargo build --release`
- Test: `cargo test` (unit + `tests/e2e.rs`, which shells out to `git` and uses
  a local fixture repo)
- Lint: `cargo clippy --all-targets`
- Format: `cargo fmt --check`

The binary is `agenv`. It shells out to the `git` CLI (required prerequisite);
the content-addressed store defaults to `~/.local/share/agenv/store/` and can be
overridden with `AGENV_STORE` for tests.

