use crate::structure::Structure;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use bincode::{serialize, deserialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum DatabaseValue {
    Int(i32),
    Float(f64),
    Text(String),
    Bool(bool),
    Null,
}

// f64 doesn't implement Eq, so we compare by bit pattern.
impl PartialEq for DatabaseValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Int(a),   Self::Int(b))   => a == b,
            (Self::Float(a), Self::Float(b)) => a.to_bits() == b.to_bits(),
            (Self::Text(a),  Self::Text(b))  => a == b,
            (Self::Bool(a),  Self::Bool(b))  => a == b,
            (Self::Null,     Self::Null)      => true,
            _                                => false,
        }
    }
}

impl Eq for DatabaseValue {}

pub struct Database {
    pub structures: HashMap<String, Structure<DatabaseValue>>,
}

impl Database {
    pub fn new() -> Self {
        Database { structures: HashMap::new() }
    }

    pub fn add_structure(&mut self, name: String, structure: Structure<DatabaseValue>) {
        self.structures.insert(name, structure);
    }

    pub fn get_structure(&self, name: &str) -> Option<&Structure<DatabaseValue>> {
        self.structures.get(name)
    }

    pub fn get_structure_mut(&mut self, name: &str) -> Option<&mut Structure<DatabaseValue>> {
        self.structures.get_mut(name)
    }

    pub fn remove_structure(&mut self, name: &str) -> bool {
        self.structures.remove(name).is_some()
    }

    // Saves the entire database to a single file.
    // Format: [u64 count] then for each structure: [framed name][framed structure bytes]
    pub fn save(&self, path: &str) -> std::io::Result<()> {
        fn write_framed(buf: &mut Vec<u8>, data: &[u8]) {
            buf.extend_from_slice(&(data.len() as u64).to_le_bytes());
            buf.extend_from_slice(data);
        }

        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(&(self.structures.len() as u64).to_le_bytes());

        for (name, structure) in &self.structures {
            write_framed(&mut buf, &serialize(name).unwrap());
            write_framed(&mut buf, &structure.to_bytes());
        }

        std::fs::write(path, buf)
    }

    pub fn load(path: &str) -> std::io::Result<Self> {
        fn read_framed<'a>(bytes: &'a [u8], offset: &mut usize) -> &'a [u8] {
            let len = u64::from_le_bytes(bytes[*offset..*offset + 8].try_into().unwrap()) as usize;
            *offset += 8;
            let data = &bytes[*offset..*offset + len];
            *offset += len;
            data
        }

        let bytes = std::fs::read(path)?;
        let mut offset = 0;

        let count = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize;
        offset += 8;

        let mut structures = HashMap::new();
        for _ in 0..count {
            let name: String = deserialize(read_framed(&bytes, &mut offset)).unwrap();
            let structure = Structure::from_bytes(read_framed(&bytes, &mut offset));
            structures.insert(name, structure);
        }

        Ok(Database { structures })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structure::Structure;
    use crate::node::NodeRef;

    fn scratch_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("maprootdb_test_db_{}_{}.bin", name, std::process::id()));
        p
    }

    #[test]
    fn round_trip_multiple_structures() {
        // Structure 1: un-strict, single node.
        let root1: NodeRef<DatabaseValue> = NodeRef::new("root1".to_string(), DatabaseValue::Int(10));
        let structure1: Structure<DatabaseValue> = Structure::new(Some(root1), "un-strict".to_string());

        // Structure 2: semi-strict, root + child with an edge.
        let root2: NodeRef<DatabaseValue> = NodeRef::new("root2".to_string(), DatabaseValue::Text("hello".to_string()));
        let mut structure2: Structure<DatabaseValue> = Structure::new(Some(root2.rc_clone()), "semi-strict".to_string());
        let child2: NodeRef<DatabaseValue> = NodeRef::new("child2".to_string(), DatabaseValue::Bool(true));
        {
            let mut root2_mut = root2.rc_clone();
            root2_mut.add_child(child2.rc_clone());
        }
        structure2.add_node(child2.rc_clone()).expect("child2 add failed");

        let mut db = Database::new();
        db.add_structure("structure_one".to_string(), structure1);
        db.add_structure("structure_two".to_string(), structure2);

        let path = scratch_path("multi_struct");
        let path_str = path.to_str().unwrap();

        db.save(path_str).expect("save failed");
        let restored = Database::load(path_str).expect("load failed");

        std::fs::remove_file(&path).ok();

        assert_eq!(restored.structures.len(), 2);

        let restored1 = restored.get_structure("structure_one").expect("structure_one missing");
        assert_eq!(restored1.mode, "un-strict");
        assert_eq!(restored1.nodes.len(), 1);
        assert_eq!(restored1.root.as_ref().unwrap().key(), "root1");
        assert_eq!(restored1.root.as_ref().unwrap().value(), DatabaseValue::Int(10));

        let restored2 = restored.get_structure("structure_two").expect("structure_two missing");
        assert_eq!(restored2.mode, "semi-strict");
        assert_eq!(restored2.nodes.len(), 2);
        let restored_root2 = restored2.find_node_by_key("root2").unwrap();
        assert_eq!(restored_root2.value(), DatabaseValue::Text("hello".to_string()));
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
}
