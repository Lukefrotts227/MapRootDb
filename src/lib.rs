//! MapRootDb is an experimental, in-memory directed acyclic graph database.
//!
//! Use [`Database`] and [`Structure`] directly in-process, or communicate with the
//! localhost server using [`Command`] and [`Response`]. Snapshot and wire formats are
//! intentionally considered unstable in the 0.1 release.

mod database;
mod node;
mod protocol;
mod server;
mod structure;

pub use database::{Database, DatabaseValue};
pub use node::NodeRef;
pub use protocol::{read_response, write_command, Command, Response};
pub use server::{bind_loopback, execute, start, start_on_listener};
pub use structure::{Structure, StructureError};
