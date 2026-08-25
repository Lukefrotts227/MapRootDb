# MapRootDb Revive — Final Report

Status: **complete**. All 13 tasks in [PROGRESS.md](PROGRESS.md) are done and committed.

## What was built

MapRootDb is a Rust in-memory database built around a Node/Structure directed-graph
data structure (`Rc<RefCell<Node<T>>>` nodes forming parent/child relationships,
grouped into named `Structure`s with "un-strict"/"semi-strict" consistency modes,
aggregated in a `Database`). It persists to disk via bincode/serde and exposes a
TCP protocol/server. This revive took it from "builds but essentially untested,
with an unexercised persistence path" to a correct, tested, warning-free state,
plus a static landing page.

Concretely:
- **Persistence made trustworthy.** `Structure::to_bytes/from_bytes/save_to_file/load_from_file`
  and `Database::save/load` were written but never exercised. Round-trip unit
  tests were added for both layers; both turned out to already be correct.
- **Two real logic bugs found and fixed** while writing tests (not hypothetical —
  both would have caused live incorrect behavior):
  1. A `RefCell` double-borrow panic in `Node::delete_node()` / `Hash`/`PartialEq`
     impls (fixed by narrowing which borrow needed to be mutable).
  2. `Structure::semi_strict_test`'s no-relation branch had its true/false
     outcomes swapped — it rejected the very first node added to an empty
     semi-strict structure and accepted later orphaned nodes, the *opposite*
     of the documented rule. Fixed with a two-line swap, verified against
     cases beyond the tests that exposed it.
- **Server no longer panics on malformed input.** `protocol.rs`'s frame/command/
  response functions now propagate `io::Error` instead of calling `.unwrap()`
  on bincode data; `server.rs` already handled this correctly; `main.rs`'s ad-hoc
  client was also converted to propagate `Result` (this was caught by critic
  review after the first pass left it out — see "What was worked around" below).
- **Dead code resolved deliberately, not silently.** `remove_node_by_key` and
  `serialize_related_ids` were confirmed unused anywhere and removed; the empty
  `puppet.rs` stub was removed along with its `mod` declaration.
- **All 14 original compiler warnings eliminated.** The root cause of most of
  them was `main.rs` re-declaring the same modules as `lib.rs` instead of
  depending on the library crate, which made real code look unused from each
  side. Unified to the idiomatic bin-depends-on-lib pattern; `cargo build`
  is now clean.
- **Ad-hoc client script no longer runs on every `cargo run`.** It's gated
  behind a `--run-client-tests` flag / `RUN_CLIENT_TESTS` env var so a plain
  `cargo run` just starts the server.
- **21 unit tests added** across node/structure/database/protocol (there were 0
  before this run).
- **Static landing page** at `docs/index.html` — self-contained, no build step,
  ready for GitHub Pages (`/docs` folder convention) — with project description,
  install/build instructions, and a link to
  https://github.com/Lukefrotts227/MapRootDb.

## What was worked around

- **T2's commit got flagged by the critic** for appearing to add ~150 lines of
  "new" persistence code under a "no functional change" warning-cleanup task.
  Escalated to judge: verified this code was pre-existing uncommitted author
  WIP (visible in prior commit messages, and depended on by T3) that simply
  got swept into the same commit as the scoped warning fixes, not new work by
  T2's builder. Ruling: no revert needed, ruling logged in PROGRESS.md.
- **T7 initially left `main.rs` still calling `.unwrap()`** on protocol reads/
  writes, against the task's explicit instruction to update call sites in both
  `server.rs` and `main.rs`. Critic flagged this (confidence 68, unresolved),
  escalated to judge, ruling required a follow-up fix — dispatched and
  completed; verified end-to-end afterward.
- **PROGRESS.md got accidentally gitignored twice** during the run (once
  caught by the judge during the T7 escalation, once as a stray uncommitted
  local edit from an agent working off stale context before T9's commit).
  Both times it was caught and fixed immediately, since it's the file this
  entire run's resumability depends on. It is currently tracked in git and
  `.gitignore` does not reference it.

## What's deferred / optional

- **Broader DAG test coverage.** Tests cover tree-shaped graphs (single parent
  per node); a reviewer during T3 noted multi-parent DAG topologies and
  dangling-reference edge cases aren't explicitly exercised. Not a known bug,
  just untested surface area.
- **Deploying `docs/index.html` to GitHub Pages** — per intake, this run stops
  at "ready to deploy"; the user deploys it themselves (point GitHub Pages at
  the `/docs` folder on the `main` branch).

## What was intentionally out of scope

- The "universal hashmap" secondary-indexing idea (a stale comment in
  `structure.rs` about indexing nodes by arbitrary fields of their value, not
  just by key) was explicitly confirmed out of scope during intake — the goal
  was to finish the existing design, not add new features. This was left
  alone / cleaned up as a stale comment, not implemented.

## Final state

- `cargo build`: 0 warnings, 0 errors.
- `cargo test`: 21/21 passing.
- All work committed to `main`, one task per commit, per PROGRESS.md's
  commit discipline.
