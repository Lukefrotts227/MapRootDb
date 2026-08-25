use serde::{Serialize, Deserialize};
use std::io::{self, Read, Write};
use crate::database::DatabaseValue;

#[derive(Serialize, Deserialize, Debug)]
pub enum Command {
    Ping,
    AddStructure { name: String, mode: String },
    AddNode      { structure: String, key: String, value: DatabaseValue },
    AddEdge      { structure: String, parent_key: String, child_key: String },
    GetNode      { structure: String, key: String },
    GetStructure { name: String },
    DeleteNode   { structure: String, key: String },
    Save         { path: String },
    Shutdown,
}

// NodeList entries: (key, value, child_keys)
#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Pong,
    Ok,
    Value(DatabaseValue),
    NodeList(Vec<(String, DatabaseValue, Vec<String>)>),
    Error(String),
}

// --- wire format: [u64 length][bytes] ---

pub fn write_frame<W: Write>(w: &mut W, data: &[u8]) -> io::Result<()> {
    w.write_all(&(data.len() as u64).to_le_bytes())?;
    w.write_all(data)
}

pub fn read_frame<R: Read>(r: &mut R) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 8];
    r.read_exact(&mut len_buf)?;
    let len = u64::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

fn bincode_err_to_io(e: bincode::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e)
}

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
        assert!(result.is_err(), "expected malformed frame to produce an Err, not panic");
    }

    #[test]
    fn valid_command_round_trips() {
        let mut buf = Vec::new();
        write_command(&mut buf, &Command::Ping).unwrap();
        let mut cursor = Cursor::new(buf);
        let cmd = read_command(&mut cursor).unwrap();
        assert!(matches!(cmd, Command::Ping));
    }
}
