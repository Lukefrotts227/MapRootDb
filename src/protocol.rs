use crate::database::DatabaseValue;
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

#[derive(Serialize, Deserialize, Debug)]
/// A request accepted by the MapRootDb TCP server.
pub enum Command {
    /// Checks that the server is responsive.
    Ping,
    /// Adds an empty named structure in `semi-strict` or `un-strict` mode.
    AddStructure { name: String, mode: String },
    /// Adds a node without relationships.
    AddNode {
        structure: String,
        key: String,
        value: DatabaseValue,
    },
    /// Atomically adds a node and its initial relationships.
    AddNodeWithEdges {
        structure: String,
        key: String,
        value: DatabaseValue,
        parent_keys: Vec<String>,
        child_keys: Vec<String>,
    },
    /// Adds a parent-to-child edge between existing nodes.
    AddEdge {
        structure: String,
        parent_key: String,
        child_key: String,
    },
    /// Gets one node's value.
    GetNode { structure: String, key: String },
    /// Gets all nodes and child keys in a structure.
    GetStructure { name: String },
    /// Deletes a node when the structure mode permits it.
    DeleteNode { structure: String, key: String },
    /// Saves the database on the server host.
    Save { path: String },
    /// Gracefully stops the server.
    Shutdown,
}

// NodeList entries: (key, value, child_keys)
#[derive(Serialize, Deserialize, Debug)]
/// A response returned by the MapRootDb TCP server.
pub enum Response {
    /// Response to [`Command::Ping`].
    Pong,
    /// A command completed successfully without a value.
    Ok,
    /// A single node value.
    Value(DatabaseValue),
    /// Structure entries represented as `(key, value, child_keys)`.
    NodeList(Vec<(String, DatabaseValue, Vec<String>)>),
    /// A human-readable command failure.
    Error(String),
}

// --- wire format: [u64 length][bytes] ---

pub const MAX_FRAME_SIZE: usize = 8 * 1024 * 1024;

pub fn write_frame<W: Write>(w: &mut W, data: &[u8]) -> io::Result<()> {
    if data.len() > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("frame exceeds {MAX_FRAME_SIZE} byte limit"),
        ));
    }
    // Send the prefix and payload together. Two tiny writes can interact badly with
    // Nagle's algorithm and delayed ACKs on persistent request/response connections.
    let mut frame = Vec::with_capacity(8 + data.len());
    frame.extend_from_slice(&(data.len() as u64).to_le_bytes());
    frame.extend_from_slice(data);
    w.write_all(&frame)
}

pub fn read_frame<R: Read>(r: &mut R) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 8];
    r.read_exact(&mut len_buf)?;
    let len = usize::try_from(u64::from_le_bytes(len_buf)).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "frame length is too large for this platform",
        )
    })?;
    if len > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame exceeds {MAX_FRAME_SIZE} byte limit"),
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

fn bincode_err_to_io(e: bincode::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e)
}

/// Serializes and writes one framed command.
pub fn write_command<W: Write>(w: &mut W, cmd: &Command) -> io::Result<()> {
    let bytes = bincode::serialize(cmd).map_err(bincode_err_to_io)?;
    write_frame(w, &bytes)
}

pub fn read_command<R: Read>(r: &mut R) -> io::Result<Command> {
    let bytes = read_frame(r)?;
    bincode::deserialize(&bytes).map_err(bincode_err_to_io)
}

pub fn write_response<W: Write>(w: &mut W, resp: &Response) -> io::Result<()> {
    let bytes = bincode::serialize(resp).map_err(bincode_err_to_io)?;
    write_frame(w, &bytes)
}

/// Reads and deserializes one framed response.
pub fn read_response<R: Read>(r: &mut R) -> io::Result<Response> {
    let bytes = read_frame(r)?;
    bincode::deserialize(&bytes).map_err(bincode_err_to_io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn malformed_frame_returns_error_not_panic() {
        // Valid length prefix but garbage bincode payload for a Command.
        let garbage = vec![0xFFu8; 16];
        let mut buf = Vec::new();
        write_frame(&mut buf, &garbage).unwrap();

        let mut cursor = Cursor::new(buf);
        let result = read_command(&mut cursor);
        assert!(
            result.is_err(),
            "expected malformed frame to produce an Err, not panic"
        );
    }

    #[test]
    fn valid_command_round_trips() {
        let mut buf = Vec::new();
        write_command(&mut buf, &Command::Ping).unwrap();
        let mut cursor = Cursor::new(buf);
        let cmd = read_command(&mut cursor).unwrap();
        assert!(matches!(cmd, Command::Ping));
    }

    #[test]
    fn oversized_frame_is_rejected_before_reading_payload() {
        let bytes = ((MAX_FRAME_SIZE as u64) + 1).to_le_bytes();
        let result = read_frame(&mut Cursor::new(bytes));

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn oversized_outgoing_frame_is_rejected() {
        let result = write_frame(&mut Vec::new(), &vec![0; MAX_FRAME_SIZE + 1]);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }
}
