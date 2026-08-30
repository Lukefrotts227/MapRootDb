# MapRootDb

[![CI](https://github.com/Lukefrotts227/MapRootDb/actions/workflows/ci.yml/badge.svg)](https://github.com/Lukefrotts227/MapRootDb/actions/workflows/ci.yml)

MapRootDb is an experimental, single-process DAG relationship store written in Rust. It stores
typed values in named directed acyclic graph structures, persists snapshots with `bincode`, and
exposes a small length-prefixed TCP protocol.

The project is currently an **alpha**. It is suitable for learning, experimentation, and
non-critical local data. The current limitations are documented below so the reliability claims
remain auditable.

## What works

- `Int`, `Float`, `Text`, `Bool`, and `Null` values
- Named graph structures with `un-strict` and `semi-strict` insertion modes
- Parent/child relationships with duplicate-edge, self-edge, and cycle rejection
- Binary database snapshots and round-trip loading
- Rejection of truncated, malformed, inconsistent, cyclic, and trailing snapshot data
- A localhost TCP server with an 8 MiB frame limit
- Commands for adding/querying structures and nodes, adding edges, deleting nodes, saving, and
  shutting down

## Quick start

Requirements: a current stable Rust toolchain.

```bash
git clone https://github.com/Lukefrotts227/MapRootDb.git
cd MapRootDb
cargo run -- demo
```

The self-contained demo starts a server, builds and queries a semi-strict graph through the TCP
API, writes `maprootdb-demo-<pid>.bin`, validates an expected error, and shuts down. Its output
ends like this:

```text
[ok] GetStructure: 2 nodes, edges correct
[ok] Save maprootdb-demo-<pid>.bin
[ok] Error on missing structure

Demo completed successfully.
```

Run a persistent local server separately with:

```bash
cargo run -- serve
```

Use `cargo run -- --help` for address, snapshot-loading, and demo-output options. The server
enforces loopback addresses because the protocol has no authentication or encryption.

In a second terminal, exercise the full TCP API and stop that server cleanly:

```bash
cargo run --example tcp_client -- 127.0.0.1:7878 tcp-client-demo.bin
```

The client example connects, pings, creates and queries a graph, saves it on the server host, and
sends `Command::Shutdown`. Ctrl+C also performs a protocol shutdown and exits cleanly.

For the in-process API, run `cargo run --example three_node_graph`. To use the unreleased library
from another Cargo project:

```toml
[dependencies]
maprootdb = { git = "https://github.com/Lukefrotts227/MapRootDb" }
```

Generate browsable API documentation with `cargo doc --no-deps --open`.

`semi-strict` structures require each node after the first to have an initial relationship. Use
`add_node_with_edges` (or `Command::AddNodeWithEdges`) to perform that insertion atomically.
`un-strict` structures also permit disconnected nodes.

## Scope and non-goals

MapRootDb is currently useful for prototypes that need a small embeddable DAG of typed values with
explicit parent/child relationships and whole-database snapshots. It is also an educational example
of keeping an `Rc<RefCell<_>>` graph on one owner thread while serving TCP clients.

It is not currently a general graph-query engine: there is no query language, path/traversal API,
secondary indexing, multi-process writer support, transaction log, or production durability claim.
Queries retrieve a node or a complete structure; callers perform higher-level traversal themselves.

## Operating and recovery behavior

- `--database PATH` means “load this existing snapshot.” A missing, malformed, or incompatible
  snapshot exits nonzero instead of silently starting empty. Omit the option to start empty.
- An occupied address produces an `AddrInUse` error. Select another loopback port with `--addr`.
- Wildcard and non-loopback addresses such as `0.0.0.0:7878` are rejected.
- Stop with Ctrl+C or `Command::Shutdown`; both complete the server loop cleanly.
- Demo filenames include the process ID by default to avoid overwriting an earlier demo. An explicit
  `--output` path may still replace an existing file.

## Verification

The same checks run in CI:

```bash
cargo fmt -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked
```

The test suite covers successful persistence round trips as well as malformed frames, oversized
frames, truncated and inconsistent snapshots, duplicate identifiers, invalid modes, graph cycles,
self-edges, and semi-strict deletion edge cases.

## Design

```text
TCP client
    -> bincode command/response frames
    -> single database-owner thread
    -> Database
       -> named Structure<DatabaseValue>
          -> keyed NodeRef graph
```

The database itself stays on one thread because graph nodes use `Rc<RefCell<_>>`. Connection
threads pass commands to the owner thread over channels, so graph references never cross thread
boundaries.

## Current limitations

MapRootDb does **not** yet claim production durability or remote-network security:

- Snapshot files do not yet have a magic header, format version, checksum, or atomic replacement.
- Parent and child links both hold strong `Rc` references, so connected nodes can remain allocated
  after their containing structure is dropped.
- The TCP API has no authentication or encryption and is intentionally bound to localhost.
- `Save` accepts a client-provided path and should not be exposed to untrusted clients.
- The server uses one thread per connection; accepted sockets have 30-second read/write timeouts.
- The wire protocol and snapshot format are not yet versioned or stable.

The next reliability milestone is versioned, checksummed, atomic snapshots with startup recovery,
followed by stronger connection and authorization controls.

## License

MIT. See [LICENSE](LICENSE).

