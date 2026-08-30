use crate::structure::Structure;
use bincode::{deserialize, serialize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize, Debug)]
/// A value stored in a MapRootDb node.
pub enum DatabaseValue {
    /// A signed 32-bit integer.
    Int(i32),
    /// A 64-bit floating-point number.
    Float(f64),
    /// UTF-8 text.
    Text(String),
    /// A Boolean value.
    Bool(bool),
    /// An explicit value with no payload.
    Null,
}

// f64 doesn't implement Eq, so we compare by bit pattern.
impl PartialEq for DatabaseValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a.to_bits() == b.to_bits(),
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Null, Self::Null) => true,
            _ => false,
        }
    }
}

impl Eq for DatabaseValue {}

/// A collection of named graph structures.
pub struct Database {
    structures: HashMap<String, Structure<DatabaseValue>>,
}

impl Default for Database {
    fn default() -> Self {
        Self::new()
    }
}

impl Database {
    /// Creates an empty database.
    pub fn new() -> Self {
        Database {
            structures: HashMap::new(),
        }
    }

    /// Adds a named structure, rejecting empty or duplicate names.
    pub fn add_structure(
        &mut self,
        name: String,
        structure: Structure<DatabaseValue>,
    ) -> Result<(), &'static str> {
        if name.is_empty() {
            return Err("structure name cannot be empty");
        }
        if self.structures.contains_key(&name) {
            return Err("structure already exists");
        }
        self.structures.insert(name, structure);
        Ok(())
    }

    /// Returns a structure by name.
    pub fn get_structure(&self, name: &str) -> Option<&Structure<DatabaseValue>> {
        self.structures.get(name)
    }

    /// Returns a mutable structure by name.
    pub fn get_structure_mut(&mut self, name: &str) -> Option<&mut Structure<DatabaseValue>> {
        self.structures.get_mut(name)
    }

    /// Removes a structure and reports whether it existed.
    pub fn remove_structure(&mut self, name: &str) -> bool {
        self.structures.remove(name).is_some()
    }

    /// Returns the number of structures.
    pub fn len(&self) -> usize {
        self.structures.len()
    }

    /// Returns whether the database has no structures.
    pub fn is_empty(&self) -> bool {
        self.structures.is_empty()
    }

    /// Saves the entire database to one snapshot file.
    ///
    /// The 0.1 snapshot format is not versioned and should not be treated as a stable
    /// interchange format.
    pub fn save(&self, path: &str) -> std::io::Result<()> {
        fn write_framed(buf: &mut Vec<u8>, data: &[u8]) {
            buf.extend_from_slice(&(data.len() as u64).to_le_bytes());
            buf.extend_from_slice(data);
        }

        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(&(self.structures.len() as u64).to_le_bytes());

        let mut structure_names: Vec<_> = self.structures.keys().collect();
        structure_names.sort();
        for name in structure_names {
            let structure = &self.structures[name];
            let serialized_name = serialize(name)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
            write_framed(&mut buf, &serialized_name);
            write_framed(&mut buf, &structure.to_bytes()?);
        }

        std::fs::write(path, buf)
    }

    /// Loads and validates a database snapshot.
    pub fn load(path: &str) -> std::io::Result<Self> {
        fn invalid_data(message: impl Into<String>) -> std::io::Error {
            std::io::Error::new(std::io::ErrorKind::InvalidData, message.into())
        }

        fn read_u64(bytes: &[u8], offset: &mut usize) -> std::io::Result<u64> {
            let end = offset
                .checked_add(8)
                .ok_or_else(|| invalid_data("database length overflow"))?;
            let value = bytes
                .get(*offset..end)
                .ok_or_else(|| invalid_data("truncated database length"))?;
            *offset = end;
            Ok(u64::from_le_bytes(
                value
                    .try_into()
                    .map_err(|_| invalid_data("invalid database length"))?,
            ))
        }

        fn read_framed<'a>(bytes: &'a [u8], offset: &mut usize) -> std::io::Result<&'a [u8]> {
            let length = usize::try_from(read_u64(bytes, offset)?)
                .map_err(|_| invalid_data("database field is too large for this platform"))?;
            let end = offset
                .checked_add(length)
                .ok_or_else(|| invalid_data("database field length overflow"))?;
            let data = bytes
                .get(*offset..end)
                .ok_or_else(|| invalid_data("truncated database field"))?;
            *offset = end;
            Ok(data)
        }

        let bytes = std::fs::read(path)?;
        let mut offset = 0;

        let count = usize::try_from(read_u64(&bytes, &mut offset)?)
            .map_err(|_| invalid_data("structure count is too large for this platform"))?;
        if count > bytes.len().saturating_sub(offset) / 16 {
            return Err(invalid_data(
                "structure count exceeds remaining database data",
            ));
        }

        let mut structures = HashMap::new();
        for _ in 0..count {
            let name: String = deserialize(read_framed(&bytes, &mut offset)?)
                .map_err(|error| invalid_data(format!("invalid structure name: {error}")))?;
            if name.is_empty() {
                return Err(invalid_data("structure name cannot be empty"));
            }
            if structures.contains_key(&name) {
                return Err(invalid_data(format!("duplicate structure name '{name}'")));
            }
            let structure = Structure::from_bytes(read_framed(&bytes, &mut offset)?)?;
            structures.insert(name, structure);
        }

        if offset != bytes.len() {
            return Err(invalid_data("trailing bytes after database"));
        }

        Ok(Database { structures })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeRef;
    use crate::structure::Structure;

    fn scratch_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "maprootdb_test_db_{}_{}.bin",
            name,
            std::process::id()
        ));
        p
    }

    #[test]
    fn round_trip_multiple_structures() {
        // Structure 1: un-strict, single node.
        let root1: NodeRef<DatabaseValue> =
            NodeRef::new("root1".to_string(), DatabaseValue::Int(10));
        let structure1: Structure<DatabaseValue> =
            Structure::new(Some(root1), "un-strict".to_string());

        // Structure 2: semi-strict, root + child with an edge.
        let root2: NodeRef<DatabaseValue> = NodeRef::new(
            "root2".to_string(),
            DatabaseValue::Text("hello".to_string()),
        );
        let mut structure2: Structure<DatabaseValue> =
            Structure::new(Some(root2.rc_clone()), "semi-strict".to_string());
        let child2: NodeRef<DatabaseValue> =
            NodeRef::new("child2".to_string(), DatabaseValue::Bool(true));
        {
            let mut root2_mut = root2.rc_clone();
            root2_mut.add_child(child2.rc_clone());
        }
        structure2
            .add_node(child2.rc_clone())
            .expect("child2 add failed");

        let mut db = Database::new();
        db.add_structure("structure_one".to_string(), structure1)
            .expect("structure_one add failed");
        db.add_structure("structure_two".to_string(), structure2)
            .expect("structure_two add failed");

        let path = scratch_path("multi_struct");
        let path_str = path.to_str().unwrap();

        db.save(path_str).expect("save failed");
        let restored = Database::load(path_str).expect("load failed");

        std::fs::remove_file(&path).ok();

        assert_eq!(restored.structures.len(), 2);

        let restored1 = restored
            .get_structure("structure_one")
            .expect("structure_one missing");
        assert_eq!(restored1.mode(), "un-strict");
        assert_eq!(restored1.len(), 1);
        assert_eq!(restored1.root().unwrap().key(), "root1");
        assert_eq!(restored1.root().unwrap().value(), DatabaseValue::Int(10));

        let restored2 = restored
            .get_structure("structure_two")
            .expect("structure_two missing");
        assert_eq!(restored2.mode(), "semi-strict");
        assert_eq!(restored2.len(), 2);
        let restored_root2 = restored2.find_node_by_key("root2").unwrap();
        assert_eq!(
            restored_root2.value(),
            DatabaseValue::Text("hello".to_string())
        );
        assert!(restored_root2.has_child_by_key("child2"));
        let restored_child2 = restored2.find_node_by_key("child2").unwrap();
        assert_eq!(restored_child2.value(), DatabaseValue::Bool(true));
        assert!(restored_child2.has_parent_by_key("root2"));
    }

    #[test]
    fn round_trip_empty_database() {
        let db = Database::new();
        let path = scratch_path("empty_db");
        let path_str = path.to_str().unwrap();

        db.save(path_str).expect("save failed");
        let restored = Database::load(path_str).expect("load failed");

        std::fs::remove_file(&path).ok();

        assert_eq!(restored.structures.len(), 0);
    }

    #[test]
    fn truncated_database_returns_error_instead_of_panicking() {
        let path = scratch_path("truncated");
        std::fs::write(&path, [1_u8, 2, 3]).expect("fixture write failed");

        let outcome = std::panic::catch_unwind(|| Database::load(path.to_str().unwrap()));
        std::fs::remove_file(&path).ok();

        assert!(outcome.is_ok(), "loading malformed data panicked");
        assert!(outcome.unwrap().is_err(), "malformed data was accepted");
    }

    #[test]
    fn database_with_trailing_bytes_is_rejected() {
        let path = scratch_path("trailing");
        let mut bytes = 0_u64.to_le_bytes().to_vec();
        bytes.extend_from_slice(b"unexpected");
        std::fs::write(&path, bytes).expect("fixture write failed");

        let outcome = std::panic::catch_unwind(|| Database::load(path.to_str().unwrap()));
        std::fs::remove_file(&path).ok();

        assert!(outcome.is_ok(), "loading malformed data panicked");
        assert!(outcome.unwrap().is_err(), "trailing data was accepted");
    }
}
