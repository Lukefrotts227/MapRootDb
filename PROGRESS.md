# MapRootDb Revive Plan

Resume instructions for a cold agent: read this file top to bottom, check `git log` for
which task IDs have already been committed (commit message should reference the task
id, e.g. "T3: ..."), find the first unchecked box, and continue from there. Do not
skip ahead. Each task should be its own commit.

Goal (from intake): finish/fix the existing Node/Structure/Database graph design
(do NOT redesign it), make it correct and complete, add unit tests for core logic,
clean up warnings, then produce a standalone static HTML landing page. Deploy target:
static site (e.g. GitHub Pages), user deploys manually — building "ready to deploy"
is sufficient. GitHub repo (from `git remote -v`): https://github.com/Lukefrotts227/MapRootDb

Global constraints (apply to every task below):
- Preserve the Node/Structure DAG design (Rc<RefCell<Node<T>>>, keyed HashMap in
  Structure, parents/children as HashSet). Do not redesign it.
- Do NOT implement the "universal hashmap" secondary-indexing idea mentioned in a
  stale comment in structure.rs (~line 9) — out of scope per intake. Just clean up
  or remove the stale comment when touched.
- Keep diffs lean/minimal — this is "finish and fix," not a rewrite.
- protocol.rs, server.rs, and the on-disk file format are all fair game to change.
- Commit after each task individually with a message referencing the task id.

---

## T1 — Repo hygiene: gitignore stray artifacts (routine)
What: Add `*.bin` (or specifically `mydb.bin`, `test_structure.bin`) to `.gitignore`,
and `git rm --cached` them if they were ever tracked (they currently show as
untracked, so likely just need ignoring). Delete the stale local files from the
working tree if they're just leftover test output, not needed fixtures.
Files: `.gitignore`, remove `mydb.bin`, `test_structure.bin` from repo root.
Done when: `git status` is clean of these files, `.gitignore` covers them, build
still succeeds.

## T2 — Fix compiler warnings: unused imports/mut (routine)
What: Clean up the genuine dead-code warnings that are just sloppiness, not
incomplete features: unused `Deserialize` import in node.rs, unused `mut` in
node.rs, unused `std::io::Write` import in structure.rs, unnecessary parens in
structure.rs, unused `mut` in structure.rs. Do NOT touch the dead_code warnings
on save/load/protocol/server functions yet — those are addressed by later tasks
that wire them up (T4, T5, T7). Only fix imports/mut/parens here.
Files: `src/node.rs`, `src/structure.rs`.
Done when: `cargo build` warning count drops for these specific items; no
functional change; `cargo build` still succeeds with 0 errors.

## T3 — Persistence round-trip unit tests: Structure to_bytes/from_bytes (high-stakes: data migration/serialization correctness)
What: This is the highest-risk correctness gap per recon. Write unit tests in
structure.rs (`#[cfg(test)] mod tests`) that build a small Structure<DatabaseValue>
(a few nodes, some edges, a root), call `to_bytes()` then `from_bytes()` (and/or
`save_to_file`/`load_from_file` with a tempfile), and assert the reconstructed
Structure has the same node keys, values, parent/child edges, root, and mode as
the original. Fix any bugs found in `to_bytes`/`from_bytes`/`from_serialized_nodes`
during this — the goal is a correct round trip, not just a test that happens to
pass. If `from_serialized_nodes` doesn't correctly rewire edges by key, fix it
there (it's the core of the persistence design, not a new feature).
Files: `src/structure.rs` (add tests, fix serialization bugs found).
Done when: `cargo test` includes and passes at least 2-3 round-trip tests
covering: empty/single-node structure, multi-node structure with edges, and
mode ("un-strict"/"semi-strict") preserved after reload. Any bug fixes are
minimal and scoped to making existing round-trip logic correct.

## T4 — Persistence round-trip unit tests: Database save/load (high-stakes: data migration/serialization correctness)
What: Depends on T3. Write unit tests in database.rs exercising `Database::save()`
then `Database::load()` with a tempfile (or a scratch path in a test-local temp
dir, cleaned up after) — build a Database with 2+ structures, save, load into a
fresh Database, assert structures/nodes/values match. Fix any bugs in save()/
load() framing (length-prefix mismatches, structure name encoding, etc.) found
along the way.
Files: `src/database.rs` (add tests, fix save/load bugs found).
Done when: `cargo test` passes a Database save/load round-trip test; data
integrity confirmed for multiple structures in one file.

## T5 — Node unit tests (routine)
What: Add unit tests in node.rs covering: add_child/add_parent wiring is
bidirectional, delete_node() removes the node from all parents' and children's
sets, has_parent_by_key/get_child_by_key work correctly, edit_value updates the
value, and serialize_node/deserialize_node round-trips a single node correctly
(fix the manual bincode framing if the round trip doesn't match — don't redesign
the framing approach, just fix bugs in it).
Files: `src/node.rs` (add tests, fix bugs found in serialize/deserialize_node
if any).
Done when: `cargo test` includes and passes these Node-level tests.

## T6 — Structure unit tests: mode logic and mutation (routine)
What: Add unit tests in structure.rs covering: "un-strict" mode allows arbitrary
add_node; "semi-strict" mode requires first node freely, then requires
subsequent nodes to attach to an existing parent/child and rejects orphaned
adds; delete_node_by_key blocks deletions that would orphan other nodes in
semi-strict mode and allows them in un-strict mode; has_first_node flag
transitions correctly on first insert.
Files: `src/structure.rs` (add tests only — do not change mode-logic behavior
unless a test reveals an actual bug, in which case fix minimally and note the
fix in the commit message).
Done when: `cargo test` passes these mode/mutation tests.

## T7 — protocol.rs: replace unwrap() with proper error handling (high-stakes: breaking change to wire format/error behavior)
What: `write_frame`/`read_frame`/`write_command`/`read_command`/`write_response`/
`read_response` currently call `.unwrap()` on bincode (de)serialization, so
malformed/corrupt data panics the server thread instead of the connection
erroring gracefully. Change these to return `Result<_, io::Error>` (or a small
local error type) propagating bincode errors instead of panicking, and update
call sites in server.rs/main.rs to handle the Result (e.g. break the connection
loop and log, rather than crash the whole process). This changes function
signatures used elsewhere, so treat it as a breaking/high-stakes change to
review carefully — trace every call site.
Files: `src/protocol.rs`, `src/server.rs`, `src/main.rs` (call-site updates only,
no protocol/command semantics change).
Done when: `cargo build` succeeds, no `.unwrap()` remains in protocol.rs's frame/
command/response functions, a malformed-frame test (or manual bad-input case)
returns an Error instead of panicking, and existing ad-hoc client flow in
main.rs (Ping/AddStructure/AddNode/etc.) still works end to end.

## T8 — Resolve unused Structure helpers: remove_node_by_key and serialize_related_ids (routine)
What: `remove_node_by_key` (drop from map, keep edges) and `serialize_related_ids`
are written but unused anywhere in the codebase or protocol. Per intake, don't
invent new features to use them. Decision: since neither is required by any
Command in protocol.rs and recon found no other caller, remove them to reduce
dead surface area UNLESS doing so would require unwinding logic other kept
methods depend on (check first — if e.g. serialize_related_nodes depends on
serialize_related_ids internally, keep it and just don't add new callers).
Record the actual decision taken in the commit message (removed vs. kept-as-is)
so a resuming agent doesn't have to re-derive it.
Files: `src/structure.rs`.
Done when: either the two methods are removed and `cargo build` has no new
warnings/errors, or a one-line comment justifies keeping them (e.g. "kept:
internal dependency of X") — one of these two outcomes must be true and stated
in the commit message.

## T9 — Resolve puppet.rs stub (routine)
What: `puppet.rs` is an empty stub described as a future "controller for
creating the database" with no current dependents. Per intake (don't invent
new scope, don't leave ambiguity unresolved), the call is: since it does
nothing and the Command/execute dispatch in server.rs already serves as the
controller layer, remove the module and its declaration from lib.rs rather
than leave a dead stub. If removing causes any hidden compile dependency,
instead leave a one-line doc comment in puppet.rs stating it's intentionally
unused and why, and note that in the commit message.
Files: `src/puppet.rs` (delete or annotate), `src/lib.rs` (remove `mod puppet;`
if deleted).
Done when: `cargo build` succeeds with the module either fully removed or
clearly annotated as intentionally empty, and lib.rs's `mod` declarations
match.

## T10 — Convert main.rs ad-hoc integration script into a real test or clearly-labeled example (routine)
What: `run_client_tests()` in main.rs is currently an ad-hoc script run inline
on every `cargo run` against a hardcoded `127.0.0.1:7878`. Since unit tests
for core logic now live in node.rs/structure.rs/database.rs (T3-T6), and
protocol/server correctness is partially covered by T7, move this client
script out of the normal startup path: either (a) gate it behind an env var/
CLI flag so `cargo run` just starts the server cleanly, or (b) move it into
`tests/integration_test.rs` as a `#[test]` using `#[ignore]` if it needs a
live server (documented as manual-run since spinning a real TCP server in
`cargo test` needs care around port reuse/test isolation — don't over-engineer
this into a full test harness). Pick whichever is the smaller diff.
Files: `src/main.rs`, optionally new `tests/integration_test.rs`.
Done when: `cargo run` starts the server without unconditionally executing the
ad-hoc client script every time, and the script itself is preserved somewhere
runnable (not deleted), either as an ignored test or a flag-gated function.

## T11 — Full warning sweep and final build check (routine)
What: After T3-T10 wire up save/load/protocol/server code that was previously
flagged dead_code, re-run `cargo build` and address any remaining warnings.
Most of the original 14 warnings should already be resolved by earlier tasks;
this task is a final sweep to catch stragglers (e.g. a helper still unused
after T8/T9 decisions, a new warning introduced by T7's error-handling change).
Files: whichever files still have warnings at this point.
Done when: `cargo build` completes with 0 warnings (or every remaining warning
is individually justified in a code comment, not silently ignored), and
`cargo test` passes all tests added in T3-T6.

## T12 — Static landing page (routine)
What: Create a standalone static HTML page (not part of the Rust build) with:
project name/description (in-memory Rust database built around a Node/Structure
DAG model with bincode persistence and a TCP protocol/server), build/install
instructions (`git clone https://github.com/Lukefrotts227/MapRootDb`, `cargo
build --release`, `cargo run`), and a link to the GitHub repo
(https://github.com/Lukefrotts227/MapRootDb — confirmed via `git remote -v`).
Keep it a single self-contained `index.html` (inline CSS is fine, no build
step, no framework) so it's directly deployable to GitHub Pages by pointing
Pages at this file/folder. Do not touch any Rust source.
Files: new `docs/index.html` (GitHub Pages' `/docs` convention — zero-config
for GitHub Pages without branch changes).
Done when: `docs/index.html` exists, opens correctly in a browser standalone
(no server needed), contains description + install instructions + a working
link to https://github.com/Lukefrotts227/MapRootDb, and no Rust files were
modified by this task.

## T13 — Final PROGRESS.md close-out (routine)
What: Once T1-T12 are all checked off, do a final pass over this file: confirm
every box is checked, add a short "Status: complete" note at the top with the
date, and leave a one-paragraph summary of what's left as genuinely optional
future work (if anything) versus what was intentionally descoped (universal
hashmap indexing — confirmed out of scope).
Files: `PROGRESS.md`.
Done when: all boxes above are checked and this closing note is added.

---

## Checklist

- [x] T1 — Repo hygiene: gitignore stray artifacts
- [x] T2 — Fix compiler warnings: unused imports/mut
- [ ] T3 — Persistence round-trip unit tests: Structure to_bytes/from_bytes (HIGH-STAKES)
- [ ] T4 — Persistence round-trip unit tests: Database save/load (HIGH-STAKES)
- [ ] T5 — Node unit tests
- [ ] T6 — Structure unit tests: mode logic and mutation
- [ ] T7 — protocol.rs: replace unwrap() with proper error handling (HIGH-STAKES)
- [ ] T8 — Resolve unused Structure helpers (remove_node_by_key, serialize_related_ids)
- [ ] T9 — Resolve puppet.rs stub
- [ ] T10 — Convert main.rs ad-hoc integration script into real test/example
- [ ] T11 — Full warning sweep and final build check
- [ ] T12 — Static landing page (docs/index.html)
- [ ] T13 — Final PROGRESS.md close-out
