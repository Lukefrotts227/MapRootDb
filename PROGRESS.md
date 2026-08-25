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

**Escalated-debate ruling (T2, commit e083764) — resolved:** Critic objected
that e083764 introduced ~150 lines of unrelated new persistence logic
(`deserialize_node`, `from_serialized_nodes`, `from_bytes`, `save_to_file`,
`load_from_file`) violating "no functional change." Verified via `git show
e083764 --stat` / `git show e083764`: this persistence code was pre-existing
uncommitted author WIP in the working tree before the revive run started
(consistent with commit 58ae561's message "need to work on node and
strucutre rebuild next will impl the file storage aspect later," and with
T3's task text below, which already assumes `from_serialized_nodes` /
`to_bytes`/`from_bytes` exist and just need tests + bugfixes — "fix it there,
it's the core of the persistence design, not a new feature"). T2's builder
did not author this code; it ran a targeted diff for imports/mut/parens but
committed the full current file contents of node.rs/structure.rs, which
swept in this already-present uncommitted functional code alongside the
intended fix. Ruling: no code change required — reverting or splitting the
commit now would only serve historical hygiene and risks destabilizing T3,
which depends on this code already existing. The commit message is
technically imprecise (claims "no functional change" while the diff shows
functional additions), which is a minor hygiene/attribution issue, not a
functional-change violation — logged here so future audits don't
re-litigate it. No follow-up builder task dispatched.

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


### ESCALATED DEBATE RULING (revive-critic confidence 68, resolved by escalation agent)
Objection: builder's commit for T7 left `.unwrap()` in main.rs's `send()` and
`run_client_tests()` (calling `write_command`/`read_response`), justifying this
in the commit message as "acceptable for a manual integration script." Critic
flagged this as an unauthorized scope narrowing since the task text explicitly
lists `src/main.rs` under Files with the instruction "update call sites in
server.rs/main.rs to handle the Result."

Steelman for builder's choice: the panics occur on a thread spawned via
`thread::spawn(run_client_tests)`, not the main server thread doing
`server::start(...)`, so a malformed/corrupt-frame panic there does not take
down the listening server -- the actual production risk T7 exists to close
(server crashing on bad input) is fully closed by the protocol.rs/server.rs
changes alone. T10 is already scheduled to restructure/relocate this exact
script into a real test or flag-gated example, so hardening its error handling
now risks being partially redone or discarded when T10 lands, in tension with
the project's lean/minimal-diff directive.

Steelman for the critic's objection: the task text is unambiguous and
specifically named -- "update call sites in server.rs/main.rs to handle the
Result" and "Files: ... src/main.rs (call-site updates only ...)" -- this is
not a case of underspecified scope where judgment calls are expected; T7 was
explicitly flagged HIGH-STAKES precisely because call sites needed to be
traced carefully, and main.rs was one of only three files named. Deciding
unilaterally that a named file doesn't need the change described is the kind
of unresolved-ambiguity call intake said not to leave to unilateral judgment,
even if the risk delta is smaller than the server-thread case.

RULING: Adopt the critic's position -- a follow-up fix to main.rs is required
before T7 can be considered fully done as specified. The task text is
explicit and named main.rs directly; "smaller blast radius" is a reasonable
mitigating factor but not authorization to silently drop a named deliverable.
The fix should stay minimal to respect the project's lean-diff directive and
avoid duplicating T10's later work: change `send()` to return
`io::Result<Response>` (or a small local error type) propagating `?` instead
of `.unwrap()`, and change `run_client_tests()` to return a `Result` that it
builds with `?`, with the `thread::spawn` call site in `main()` matching on
the result and printing an error via `eprintln!` instead of letting a panic
propagate. This satisfies the literal task text ("handle the Result") without
requiring the fuller test/example restructuring reserved for T10. Until this
follow-up lands, checkbox `[x] T7` below should be treated as
provisional/incomplete for the main.rs portion -- a resuming agent should run
this fix before marking T7 fully closed.
FOLLOW-UP NEEDED (dispatch to revive-builder): edit `src/main.rs` -- change
`send()` and `run_client_tests()` signatures to propagate `Result` instead of
`.unwrap()`-ing, and update the `thread::spawn(run_client_tests)` call site in
`main()` to handle the `Result` (log via `eprintln!` on `Err` rather than
panic). No changes needed to `src/protocol.rs` or `src/server.rs` -- both were
independently confirmed correct by the critic.

FOLLOW-UP COMPLETE (2026-08-25): `send()` now returns `std::io::Result<Response>`
and propagates errors from `write_command`/`read_response` via `?`.
`run_client_tests()` now returns `std::io::Result<()>` and propagates errors
from every `send()` call via `?` instead of `.unwrap()`. The `thread::spawn`
call site in `main()` now wraps the call in a closure that matches on the
`Result` and prints via `eprintln!` on `Err` rather than letting a panic
propagate. `src/protocol.rs` and `src/server.rs` were not touched. Verified
with `cargo build`, `cargo test` (21 passed in both lib and bin test
binaries), and `cargo run` (full Ping/AddStructure/AddNode/AddEdge/GetNode/
GetStructure/Save/error-case client flow completed successfully end-to-end).
The T7 checkbox below can now be treated as fully closed, not provisional.

### SEPARATE URGENT FINDING (independently verified, not part of T7 scope)
`.gitignore` line 4 adds `PROGRESS.md`, and commit `39c79df "stop tracking
PROGRESS.md"` already `git rm --cached`'d it -- confirmed via
`git check-ignore -v PROGRESS.md` (matches `.gitignore:4:PROGRESS.md`) and
`git ls-files | grep -i progress` (empty, i.e. untracked). The file still
exists on disk with current content, and `git log -- PROGRESS.md` shows no
commits since `39c79df` untracked it -- so all edits made to this file from
that commit onward (including this ruling) are NOT being captured by git at
all. This directly undermines the project's explicit requirement that
PROGRESS.md survive a handoff to a different agent/tool/session: anyone
resuming via `git clone`/`git checkout` of this repo will get a repo with NO
PROGRESS.md, silently. THIS NEEDS AN IMMEDIATE FIX regardless of the T7
ruling above: remove the `PROGRESS.md` line from `.gitignore` and `git add
PROGRESS.md` to resume tracking it (a plain add/commit, not a code change --
can be done directly rather than needing a builder dispatch, but flagging
here since it was out of this ruling's original scope and must not be missed
on next resume).

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
- [x] T3 — Persistence round-trip unit tests: Structure to_bytes/from_bytes (HIGH-STAKES)
- [x] T4 — Persistence round-trip unit tests: Database save/load (HIGH-STAKES)
- [x] T5 — Node unit tests
- [x] T6 — Structure unit tests: mode logic and mutation
- [x] T7 — protocol.rs: replace unwrap() with proper error handling (HIGH-STAKES)
- [x] T8 — Resolve unused Structure helpers (remove_node_by_key, serialize_related_ids)
- [ ] T9 — Resolve puppet.rs stub
- [ ] T10 — Convert main.rs ad-hoc integration script into real test/example
- [ ] T11 — Full warning sweep and final build check
- [ ] T12 — Static landing page (docs/index.html)
- [ ] T13 — Final PROGRESS.md close-out
