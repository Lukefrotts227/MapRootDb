use crate::node::NodeRef;
use bincode::{deserialize, serialize};
use serde::de::DeserializeOwned;
use serde::ser::Serialize;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io;

#[derive(Clone, Debug, PartialEq, Eq)]
/// A validation failure while constructing or mutating a structure.
pub enum StructureError {
    UnknownMode(String),
    EmptyNodeKey,
    DuplicateNodeKey(String),
    ModeViolation,
    MissingParent(String),
    MissingChild(String),
    SelfEdge,
    DuplicateEdge,
    DuplicateRelationship,
    Cycle,
}

impl fmt::Display for StructureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMode(mode) => write!(formatter, "unknown structure mode '{mode}'"),
            Self::EmptyNodeKey => write!(formatter, "node key cannot be empty"),
            Self::DuplicateNodeKey(key) => write!(formatter, "node key '{key}' already exists"),
            Self::ModeViolation => write!(
                formatter,
                "node requires an initial relationship in semi-strict mode"
            ),
            Self::MissingParent(key) => write!(formatter, "parent node '{key}' not found"),
            Self::MissingChild(key) => write!(formatter, "child node '{key}' not found"),
            Self::SelfEdge => write!(formatter, "self-edges are not allowed"),
            Self::DuplicateEdge => write!(formatter, "edge already exists"),
            Self::DuplicateRelationship => write!(formatter, "duplicate initial relationship"),
            Self::Cycle => write!(formatter, "relationship would create a cycle"),
        }
    }
}

impl std::error::Error for StructureError {}

/// A named collection's nodes and DAG relationships.
///
/// `semi-strict` requires every node after the first to be inserted with at least one
/// relationship; `un-strict` permits disconnected nodes.
pub struct Structure<T: Clone> {
    root: Option<NodeRef<T>>,
    nodes: HashMap<String, NodeRef<T>>,
    mode: String,
    has_first_node: bool,
}

impl<T: Clone + Eq + Serialize> Structure<T> {
    /// Creates a structure after validating the mode and optional root.
    pub fn try_new(root: Option<NodeRef<T>>, mode: String) -> Result<Self, StructureError> {
        if !matches!(mode.as_str(), "semi-strict" | "un-strict") {
            return Err(StructureError::UnknownMode(mode));
        }
        if root.as_ref().is_some_and(|node| node.key().is_empty()) {
            return Err(StructureError::EmptyNodeKey);
        }

        Ok(Self::new(root, mode))
    }

    pub(crate) fn new(root: Option<NodeRef<T>>, mode: String) -> Self {
        let mut nodes: HashMap<String, NodeRef<T>> = HashMap::new();
        let mut has_first_node: bool = false;

        if let Some(root) = &root {
            nodes.insert(root.key(), root.rc_clone()); // Insert root node
            has_first_node = true;
        }

        // iterate through the alt_nodes and store all the keys that will be used to hash to the node

        Structure {
            root,
            nodes,
            mode,
            has_first_node,
        }
    }

    /// Returns the number of nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns whether this structure has no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns `semi-strict` or `un-strict`.
    pub fn mode(&self) -> &str {
        &self.mode
    }

    /// Returns the root reference, if one is assigned.
    pub fn root(&self) -> Option<NodeRef<T>> {
        self.root.as_ref().map(NodeRef::rc_clone)
    }

    /// Reports whether a first node has been inserted.
    pub fn has_first_node(&self) -> bool {
        self.has_first_node
    }

    /// Iterates over node keys and references in unspecified order.
    pub fn iter_nodes(&self) -> impl Iterator<Item = (&String, &NodeRef<T>)> {
        self.nodes.iter()
    }

    // for the add node method we will have 2 modes semi-strict and un-strict,
    // semi strict means at least one parent or child must be present in the structure, unless the node is the first node in the structure
    // un-strict means that the node can be added without any parents or children
    // more modes will be added but this is good to get it going

    fn serialize_related_nodes(&self) -> io::Result<Vec<Vec<u8>>> {
        // we can use the seralize function that I wrote for indiv nodes
        let mut over_vector: Vec<Vec<u8>> = Vec::new();

        // iterate through the map to get the vectors for each node
        let mut node_keys: Vec<_> = self.nodes.keys().collect();
        node_keys.sort();
        for key in node_keys {
            let node = &self.nodes[key];
            // clone the reference
            let n: NodeRef<T> = node.rc_clone();
            let serialized: Vec<u8> = n.serialize_node()?;
            over_vector.push(serialized);
        }
        Ok(over_vector)
    }

    fn semi_strict_check_for_one(&self, node: NodeRef<T>, off_limit_key: &str) -> bool {
        let node_children: std::cell::Ref<'_, std::collections::HashSet<NodeRef<T>>> =
            node.children();
        let node_parents: std::cell::Ref<'_, std::collections::HashSet<NodeRef<T>>> =
            node.parents();
        // iterate through node children looking for a valid node exit with true if found
        // rembember to exclude the off limit key
        for child in node_children.iter() {
            // if the key is part of the structure and is not the off limit key return true
            if child.key() != off_limit_key && self.nodes.contains_key(&child.key()) {
                return true;
            }
        }
        // do the same for the parents
        for parent in node_parents.iter() {
            // if the key is part of the structure and is not the off limit key return true
            if parent.key() != off_limit_key && self.nodes.contains_key(&parent.key()) {
                return true;
            }
        }
        // if all else fails then return false
        false
    }

    pub fn delete_node_by_key(&mut self, key: &str) -> bool {
        // remove a node from the structure by key
        // must also delete the node from the parents and children of other nodes
        // must also also delete the node from the root if it is the root
        // finally use the node module to fully delete the node
        // the delete method from the node module will handle the deletion of the node from itself and its children and parents
        // also must see if the operaton breaks the current strictness of the structure
        // if the structure is semi-strict and the node being deleted is the last node in the structure then the structure will no longer be semi-strict and the deletion will fail with a return of false
        // return false if the node is not found

        // grab all the possible hashmaps from the hashmap of hashmaps

        let Some(mut node) = self.find_node_by_key(key) else {
            return false;
        };
        if self.mode == "un-strict" {
            self.nodes.remove(key);
            if self.root.as_ref().is_some_and(|root| root.key() == key) {
                self.root = None;
            }
            if self.nodes.is_empty() {
                self.has_first_node = false;
            }
            node.delete_node();
            return true;
        }

        // if the we are in a semi-strict db and the node is the last node then the removal is valid
        let parents: std::cell::Ref<'_, std::collections::HashSet<NodeRef<T>>> = node.parents();
        let children: std::cell::Ref<'_, std::collections::HashSet<NodeRef<T>>> = node.children();

        if parents.is_empty() && children.is_empty() && !self.has_first_node {
            return false;
        } else if parents.is_empty() && children.is_empty() && self.has_first_node {
            drop(parents);
            drop(children);
            self.nodes.remove(key);
            if self.root.as_ref().is_some_and(|root| root.key() == key) {
                self.root = None;
            }
            self.has_first_node = false;
            node.delete_node();
            return true;
        }

        // do the semi-strict test on the node being removed
        // start by iterating throught the parents and children of the node being removed and check for strictness without including the node being removed
        // if at any point the strictness is broken return false immediately

        // parent check
        for parent in parents.iter() {
            // make sure the parent has at lease one valid child or parent not including the node being removed
            // use the semi_strict_check_for_one method to check for at least one valid parent or child
            if !self.semi_strict_check_for_one(parent.rc_clone(), key) {
                return false;
            }
        }

        // child check
        for child in children.iter() {
            // make sure the child has at lease one valid child or parent not including the node being removed
            // use the semi_strict_check_for_one method to check for at least one valid parent or child
            if !self.semi_strict_check_for_one(child.rc_clone(), key) {
                return false;
            }
        }

        // if the semi-stric test passes it is safe to remove the node from the structure
        self.nodes.remove(key);
        if self.root.as_ref().is_some_and(|root| root.key() == key) {
            self.root = None;
        }
        if self.nodes.is_empty() {
            self.has_first_node = false;
        }
        // use borrow checker shenanigans to delete the node
        let mut other_same_node = node.rc_clone();
        other_same_node.delete_node();
        true
    }
    /// Returns the node with `key`, if it exists.
    ///
    /// The returned [`NodeRef`] points to the same node held by the structure.
    pub fn find_node_by_key(&self, key: &str) -> Option<NodeRef<T>> {
        // find a node in the structure by key using the hashmap
        // return the reference to the node if found
        // return None if not found
        self.nodes.get(key).map(|node| node.rc_clone())
    }

    /// Adds a directed parent-to-child edge between two existing nodes.
    ///
    /// Self-edges, duplicate edges, missing nodes, and edges that create cycles are
    /// rejected without changing the structure.
    pub fn add_edge_by_key(
        &mut self,
        parent_key: &str,
        child_key: &str,
    ) -> Result<(), StructureError> {
        if parent_key == child_key {
            return Err(StructureError::SelfEdge);
        }

        let mut parent = self
            .find_node_by_key(parent_key)
            .ok_or_else(|| StructureError::MissingParent(parent_key.to_string()))?;
        let child = self
            .find_node_by_key(child_key)
            .ok_or_else(|| StructureError::MissingChild(child_key.to_string()))?;

        if parent.has_child_by_key(child_key) {
            return Err(StructureError::DuplicateEdge);
        }
        if self.has_path(child_key, parent_key) {
            return Err(StructureError::Cycle);
        }

        parent.add_child(child);
        Ok(())
    }

    /// Atomically inserts a node together with its initial relationships.
    ///
    /// Use this for every node after the first in `semi-strict` mode. All referenced
    /// parents and children must already exist. Validation completes before mutation,
    /// so an error leaves the structure unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`StructureError`] for invalid keys, missing endpoints, duplicate
    /// relationships, mode violations, self-edges, or cycles.
    pub fn add_node_with_edges(
        &mut self,
        node: NodeRef<T>,
        parent_keys: &[String],
        child_keys: &[String],
    ) -> Result<NodeRef<T>, StructureError> {
        let key = node.key();
        if key.is_empty() {
            return Err(StructureError::EmptyNodeKey);
        }
        if self.nodes.contains_key(&key) {
            return Err(StructureError::DuplicateNodeKey(key));
        }
        if parent_keys.is_empty() && child_keys.is_empty() {
            return self.add_node(node);
        }

        let unique_parents: HashSet<_> = parent_keys.iter().collect();
        let unique_children: HashSet<_> = child_keys.iter().collect();
        if unique_parents.len() != parent_keys.len() || unique_children.len() != child_keys.len() {
            return Err(StructureError::DuplicateRelationship);
        }
        if parent_keys.iter().any(|parent| parent == &key)
            || child_keys.iter().any(|child| child == &key)
        {
            return Err(StructureError::SelfEdge);
        }
        if let Some(parent) = parent_keys
            .iter()
            .find(|parent| !self.nodes.contains_key(*parent))
        {
            return Err(StructureError::MissingParent(parent.clone()));
        }
        if let Some(child) = child_keys
            .iter()
            .find(|child| !self.nodes.contains_key(*child))
        {
            return Err(StructureError::MissingChild(child.clone()));
        }
        if child_keys.iter().any(|child| {
            parent_keys
                .iter()
                .any(|parent| self.has_path(child, parent))
        }) {
            return Err(StructureError::Cycle);
        }

        self.nodes.insert(key, node.rc_clone());
        self.has_first_node = true;

        for parent_key in parent_keys {
            let mut parent = self
                .find_node_by_key(parent_key)
                .expect("validated parent disappeared before edge insertion");
            parent.add_child(node.rc_clone());
        }
        let mut inserted = node.rc_clone();
        for child_key in child_keys {
            let child = self
                .find_node_by_key(child_key)
                .expect("validated child disappeared before edge insertion");
            inserted.add_child(child);
        }

        Ok(node)
    }

    fn has_path(&self, start_key: &str, target_key: &str) -> bool {
        let mut pending = vec![start_key.to_string()];
        let mut visited = HashSet::new();

        while let Some(key) = pending.pop() {
            if key == target_key {
                return true;
            }
            if !visited.insert(key.clone()) {
                continue;
            }
            if let Some(node) = self.nodes.get(&key) {
                pending.extend(
                    node.children()
                        .iter()
                        .map(|child| child.key())
                        .filter(|child_key| self.nodes.contains_key(child_key)),
                );
            }
        }

        false
    }

    fn contains_cycle(&self) -> bool {
        self.nodes.iter().any(|(parent_key, node)| {
            node.children()
                .iter()
                .any(|child| self.has_path(&child.key(), parent_key))
        })
    }

    /// Inserts an unconnected node.
    ///
    /// In `semi-strict` mode this accepts the first node only; subsequent nodes must use
    /// [`Self::add_node_with_edges`].
    pub fn add_node(&mut self, node: NodeRef<T>) -> Result<NodeRef<T>, StructureError> {
        let key = node.key();
        if key.is_empty() {
            return Err(StructureError::EmptyNodeKey);
        }
        if self.nodes.contains_key(&key) {
            return Err(StructureError::DuplicateNodeKey(key));
        }

        // depending on the mode use the correct add method
        match self.mode.as_str() {
            "semi-strict" => self.semi_strict_add(node),
            "un-strict" => self.un_strict_add(node),
            _ => Err(StructureError::UnknownMode(self.mode.clone())),
        }
    }

    fn semi_strict_add(&mut self, node: NodeRef<T>) -> Result<NodeRef<T>, StructureError> {
        // perform a semi-strict test on the node to see if it can be added to the structure
        if self.semi_strict_test(node.rc_clone()) {
            self.nodes.insert(node.key(), node.rc_clone());
            self.has_first_node = true;
            Ok(node)
        } else {
            Err(StructureError::ModeViolation)
        }
    }

    fn un_strict_add(&mut self, node: NodeRef<T>) -> Result<NodeRef<T>, StructureError> {
        // simply add the node to the structure
        self.nodes.insert(node.key(), node.rc_clone());
        self.has_first_node = true;
        Ok(node)
    }

    fn semi_strict_test(&self, node: NodeRef<T>) -> bool {
        // test if the node has at least one parent or child in the structure
        let parents = node.parents();
        let children = node.children();

        if parents.is_empty() && children.is_empty() && !self.has_first_node {
            return true;
        } else if parents.is_empty() && children.is_empty() && self.has_first_node {
            return false;
        }

        // iterate through the parents hashset and if the parent is in the structure return true

        for parent in parents.iter() {
            if self.nodes.contains_key(&parent.key()) {
                return true;
            }
        }

        // iterate through the children hashset and if the child is in the structure return true
        for child in children.iter() {
            if self.nodes.contains_key(&child.key()) {
                return true;
            }
        }

        // if nothing has been found return false
        false
    }
}

impl<T: Clone + Eq + Serialize + DeserializeOwned> Structure<T> {
    // Rebuild a Structure from the raw serialized node blobs produced by serialize_related_nodes.
    pub fn from_serialized_nodes(
        serialized_nodes: Vec<Vec<u8>>,
        root_key: Option<String>,
        mode: String,
    ) -> io::Result<Self> {
        fn invalid_data(message: impl Into<String>) -> io::Error {
            io::Error::new(io::ErrorKind::InvalidData, message.into())
        }

        if !matches!(mode.as_str(), "semi-strict" | "un-strict") {
            return Err(invalid_data(format!("unknown structure mode '{mode}'")));
        }

        let mut node_map: HashMap<String, NodeRef<T>> = HashMap::new();
        let mut relationships: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();

        for node_bytes in serialized_nodes {
            let (node_ref, parent_keys, child_keys) = NodeRef::deserialize_node(&node_bytes)?;
            let key = node_ref.key();
            if key.is_empty() {
                return Err(invalid_data("node key cannot be empty"));
            }
            if node_map.contains_key(&key) {
                return Err(invalid_data(format!("duplicate node key '{key}'")));
            }
            node_map.insert(key.clone(), node_ref);
            relationships.insert(key, (parent_keys, child_keys));
        }

        for (key, (parent_keys, child_keys)) in &relationships {
            let unique_parents: HashSet<_> = parent_keys.iter().collect();
            let unique_children: HashSet<_> = child_keys.iter().collect();
            if unique_parents.len() != parent_keys.len()
                || unique_children.len() != child_keys.len()
            {
                return Err(invalid_data(format!(
                    "duplicate relationship for node '{key}'"
                )));
            }
            if parent_keys.iter().any(|parent| parent == key)
                || child_keys.iter().any(|child| child == key)
            {
                return Err(invalid_data(format!("self-edge for node '{key}'")));
            }

            for parent_key in parent_keys {
                let (_, parent_children) = relationships.get(parent_key).ok_or_else(|| {
                    invalid_data(format!(
                        "node '{key}' references missing parent '{parent_key}'"
                    ))
                })?;
                if !parent_children.contains(key) {
                    return Err(invalid_data(format!(
                        "inconsistent edge between '{parent_key}' and '{key}'"
                    )));
                }
            }
            for child_key in child_keys {
                let (child_parents, _) = relationships.get(child_key).ok_or_else(|| {
                    invalid_data(format!(
                        "node '{key}' references missing child '{child_key}'"
                    ))
                })?;
                if !child_parents.contains(key) {
                    return Err(invalid_data(format!(
                        "inconsistent edge between '{key}' and '{child_key}'"
                    )));
                }
            }
        }

        // Wire up edges - add_child sets both sides, so only process children to avoid double-linking.
        for (node_key, (_, child_keys)) in &relationships {
            let mut node = node_map
                .get(node_key)
                .ok_or_else(|| invalid_data("relationship references a missing node"))?
                .rc_clone();
            for child_key in child_keys {
                let child = node_map
                    .get(child_key)
                    .ok_or_else(|| invalid_data("relationship references a missing child"))?;
                node.add_child(child.rc_clone());
            }
        }

        let root = match root_key {
            Some(key) => Some(
                node_map
                    .get(&key)
                    .ok_or_else(|| invalid_data(format!("root node '{key}' is missing")))?
                    .rc_clone(),
            ),
            None => None,
        };
        let has_first_node = !node_map.is_empty();
        let structure = Structure {
            root,
            nodes: node_map,
            mode,
            has_first_node,
        };
        if structure.contains_cycle() {
            return Err(invalid_data("structure contains a cycle"));
        }
        Ok(structure)
    }

    pub fn to_bytes(&self) -> io::Result<Vec<u8>> {
        fn write_framed(buf: &mut Vec<u8>, data: &[u8]) {
            buf.extend_from_slice(&(data.len() as u64).to_le_bytes());
            buf.extend_from_slice(data);
        }

        fn serialization_error(error: bincode::Error) -> io::Error {
            io::Error::new(io::ErrorKind::InvalidData, error)
        }

        let mut buf = Vec::new();
        write_framed(
            &mut buf,
            &serialize(&self.root.as_ref().map(|r| r.key())).map_err(serialization_error)?,
        );
        write_framed(
            &mut buf,
            &serialize(&self.mode).map_err(serialization_error)?,
        );

        let node_blobs = self.serialize_related_nodes()?;
        buf.extend_from_slice(&(node_blobs.len() as u64).to_le_bytes());
        for blob in node_blobs {
            write_framed(&mut buf, &blob);
        }

        Ok(buf)
    }

    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        fn invalid_data(message: impl Into<String>) -> io::Error {
            io::Error::new(io::ErrorKind::InvalidData, message.into())
        }

        fn read_u64(bytes: &[u8], offset: &mut usize) -> io::Result<u64> {
            let end = offset
                .checked_add(8)
                .ok_or_else(|| invalid_data("structure length overflow"))?;
            let value = bytes
                .get(*offset..end)
                .ok_or_else(|| invalid_data("truncated structure length"))?;
            *offset = end;
            Ok(u64::from_le_bytes(
                value
                    .try_into()
                    .map_err(|_| invalid_data("invalid structure length"))?,
            ))
        }

        fn read_framed<'a>(bytes: &'a [u8], offset: &mut usize) -> io::Result<&'a [u8]> {
            let length = usize::try_from(read_u64(bytes, offset)?)
                .map_err(|_| invalid_data("structure field is too large for this platform"))?;
            let end = offset
                .checked_add(length)
                .ok_or_else(|| invalid_data("structure field length overflow"))?;
            let data = bytes
                .get(*offset..end)
                .ok_or_else(|| invalid_data("truncated structure field"))?;
            *offset = end;
            Ok(data)
        }

        let mut offset = 0;
        let root_key: Option<String> = deserialize(read_framed(bytes, &mut offset)?)
            .map_err(|error| invalid_data(format!("invalid structure root: {error}")))?;
        let mode: String = deserialize(read_framed(bytes, &mut offset)?)
            .map_err(|error| invalid_data(format!("invalid structure mode: {error}")))?;
        let node_count = usize::try_from(read_u64(bytes, &mut offset)?)
            .map_err(|_| invalid_data("node count is too large for this platform"))?;
        if node_count > bytes.len().saturating_sub(offset) / 8 {
            return Err(invalid_data("node count exceeds remaining structure data"));
        }

        let mut serialized_nodes = Vec::new();
        for _ in 0..node_count {
            serialized_nodes.push(read_framed(bytes, &mut offset)?.to_vec());
        }

        if offset != bytes.len() {
            return Err(invalid_data("trailing bytes after structure"));
        }

        Self::from_serialized_nodes(serialized_nodes, root_key, mode)
    }

    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        std::fs::write(path, self.to_bytes()?)
    }

    pub fn load_from_file(path: &str) -> std::io::Result<Self> {
        Self::from_bytes(&std::fs::read(path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DatabaseValue;
    use crate::node::NodeRef;

    fn scratch_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "maprootdb_test_{}_{}.bin",
            name,
            std::process::id()
        ));
        p
    }

    #[test]
    fn round_trip_empty_structure() {
        let structure: Structure<DatabaseValue> = Structure::new(None, "un-strict".to_string());

        let bytes = structure.to_bytes().expect("serialize failed");
        let restored: Structure<DatabaseValue> =
            Structure::from_bytes(&bytes).expect("deserialize failed");

        assert!(restored.root.is_none());
        assert_eq!(restored.nodes.len(), 0);
        assert_eq!(restored.mode, "un-strict");
        assert!(!restored.has_first_node);
    }

    #[test]
    fn round_trip_single_node() {
        let root: NodeRef<DatabaseValue> = NodeRef::new("root".to_string(), DatabaseValue::Int(42));
        let structure: Structure<DatabaseValue> =
            Structure::new(Some(root), "un-strict".to_string());

        let bytes = structure.to_bytes().expect("serialize failed");
        let restored: Structure<DatabaseValue> =
            Structure::from_bytes(&bytes).expect("deserialize failed");

        assert_eq!(restored.nodes.len(), 1);
        assert_eq!(restored.mode, "un-strict");
        assert!(restored.has_first_node);
        assert!(restored.root.is_some());
        assert_eq!(restored.root.as_ref().unwrap().key(), "root");
        assert_eq!(
            restored.root.as_ref().unwrap().value(),
            DatabaseValue::Int(42)
        );
    }

    #[test]
    fn round_trip_multi_node_with_edges() {
        let root: NodeRef<DatabaseValue> = NodeRef::new(
            "root".to_string(),
            DatabaseValue::Text("root-val".to_string()),
        );
        let mut structure: Structure<DatabaseValue> =
            Structure::new(Some(root.rc_clone()), "semi-strict".to_string());

        let mut child_a: NodeRef<DatabaseValue> =
            NodeRef::new("child_a".to_string(), DatabaseValue::Int(1));
        let child_b: NodeRef<DatabaseValue> =
            NodeRef::new("child_b".to_string(), DatabaseValue::Bool(true));
        let grandchild: NodeRef<DatabaseValue> =
            NodeRef::new("grandchild".to_string(), DatabaseValue::Float(3.5));

        // root -> child_a, root -> child_b, child_a -> grandchild
        {
            let mut root_mut = root.rc_clone();
            root_mut.add_child(child_a.rc_clone());
            root_mut.add_child(child_b.rc_clone());
        }
        child_a.add_child(grandchild.rc_clone());

        structure
            .add_node(child_a.rc_clone())
            .expect("child_a add failed");
        structure
            .add_node(child_b.rc_clone())
            .expect("child_b add failed");
        structure
            .add_node(grandchild.rc_clone())
            .expect("grandchild add failed");

        let bytes = structure.to_bytes().expect("serialize failed");
        let restored: Structure<DatabaseValue> =
            Structure::from_bytes(&bytes).expect("deserialize failed");

        assert_eq!(restored.mode, "semi-strict");
        assert!(restored.has_first_node);
        assert_eq!(restored.nodes.len(), 4);
        assert_eq!(restored.root.as_ref().unwrap().key(), "root");

        // check values
        assert_eq!(
            restored.find_node_by_key("root").unwrap().value(),
            DatabaseValue::Text("root-val".to_string())
        );
        assert_eq!(
            restored.find_node_by_key("child_a").unwrap().value(),
            DatabaseValue::Int(1)
        );
        assert_eq!(
            restored.find_node_by_key("child_b").unwrap().value(),
            DatabaseValue::Bool(true)
        );
        assert_eq!(
            restored.find_node_by_key("grandchild").unwrap().value(),
            DatabaseValue::Float(3.5)
        );

        // check edges: root -> child_a, child_b
        let restored_root = restored.find_node_by_key("root").unwrap();
        assert!(restored_root.has_child_by_key("child_a"));
        assert!(restored_root.has_child_by_key("child_b"));
        assert_eq!(restored_root.children().len(), 2);
        assert_eq!(restored_root.parents().len(), 0);

        // check reverse edges wired correctly
        let restored_child_a = restored.find_node_by_key("child_a").unwrap();
        assert!(restored_child_a.has_parent_by_key("root"));
        assert!(restored_child_a.has_child_by_key("grandchild"));

        let restored_child_b = restored.find_node_by_key("child_b").unwrap();
        assert!(restored_child_b.has_parent_by_key("root"));
        assert_eq!(restored_child_b.children().len(), 0);

        let restored_grandchild = restored.find_node_by_key("grandchild").unwrap();
        assert!(restored_grandchild.has_parent_by_key("child_a"));
        assert_eq!(restored_grandchild.children().len(), 0);
    }

    #[test]
    fn round_trip_via_file_preserves_mode_and_edges() {
        let root: NodeRef<DatabaseValue> = NodeRef::new("root".to_string(), DatabaseValue::Null);
        let mut structure: Structure<DatabaseValue> =
            Structure::new(Some(root.rc_clone()), "semi-strict".to_string());

        let child: NodeRef<DatabaseValue> =
            NodeRef::new("child".to_string(), DatabaseValue::Int(7));
        {
            let mut root_mut = root.rc_clone();
            root_mut.add_child(child.rc_clone());
        }
        structure
            .add_node(child.rc_clone())
            .expect("child add failed");

        let path = scratch_path("structure_file_roundtrip");
        let path_str = path.to_str().unwrap();

        structure
            .save_to_file(path_str)
            .expect("save_to_file failed");
        let restored: Structure<DatabaseValue> =
            Structure::load_from_file(path_str).expect("load_from_file failed");

        std::fs::remove_file(&path).ok();

        assert_eq!(restored.mode, "semi-strict");
        assert_eq!(restored.nodes.len(), 2);
        assert_eq!(restored.root.as_ref().unwrap().key(), "root");
        assert!(restored
            .find_node_by_key("root")
            .unwrap()
            .has_child_by_key("child"));
        assert!(restored
            .find_node_by_key("child")
            .unwrap()
            .has_parent_by_key("root"));
    }

    #[test]
    fn un_strict_allows_arbitrary_add_node() {
        let mut structure: Structure<DatabaseValue> = Structure::new(None, "un-strict".to_string());
        assert!(!structure.has_first_node);

        // first node, no parents/children
        let a: NodeRef<DatabaseValue> = NodeRef::new("a".to_string(), DatabaseValue::Int(1));
        assert!(structure.add_node(a.rc_clone()).is_ok());
        assert!(structure.has_first_node);

        // second node, completely unrelated to anything in the structure
        let b: NodeRef<DatabaseValue> = NodeRef::new("b".to_string(), DatabaseValue::Int(2));
        assert!(structure.add_node(b.rc_clone()).is_ok());
        assert_eq!(structure.nodes.len(), 2);
    }

    #[test]
    fn public_constructor_rejects_invalid_mode_and_empty_root_key() {
        assert!(Structure::<DatabaseValue>::try_new(None, "typo".to_string()).is_err());

        let empty_root = NodeRef::new(String::new(), DatabaseValue::Null);
        assert!(Structure::try_new(Some(empty_root), "un-strict".to_string()).is_err());
    }

    #[test]
    fn semi_strict_allows_first_node_freely() {
        let mut structure: Structure<DatabaseValue> =
            Structure::new(None, "semi-strict".to_string());
        assert!(!structure.has_first_node);

        let a: NodeRef<DatabaseValue> = NodeRef::new("a".to_string(), DatabaseValue::Int(1));
        let result = structure.add_node(a.rc_clone());
        assert!(result.is_ok());
        assert!(structure.has_first_node);
        assert_eq!(structure.nodes.len(), 1);
    }

    #[test]
    fn semi_strict_rejects_orphaned_add_after_first_node() {
        let mut structure: Structure<DatabaseValue> =
            Structure::new(None, "semi-strict".to_string());

        let a: NodeRef<DatabaseValue> = NodeRef::new("a".to_string(), DatabaseValue::Int(1));
        structure
            .add_node(a.rc_clone())
            .expect("first node add should succeed");

        // orphan node: no parents or children linking it into the structure
        let b: NodeRef<DatabaseValue> = NodeRef::new("b".to_string(), DatabaseValue::Int(2));
        let result = structure.add_node(b.rc_clone());
        assert!(result.is_err());
        assert_eq!(structure.nodes.len(), 1);
        assert!(structure.find_node_by_key("b").is_none());
    }

    #[test]
    fn semi_strict_allows_add_with_existing_parent_link() {
        let mut structure: Structure<DatabaseValue> =
            Structure::new(None, "semi-strict".to_string());

        let a: NodeRef<DatabaseValue> = NodeRef::new("a".to_string(), DatabaseValue::Int(1));
        structure
            .add_node(a.rc_clone())
            .expect("first node add should succeed");

        // b is linked to a (already in structure), so it should be accepted
        let b: NodeRef<DatabaseValue> = NodeRef::new("b".to_string(), DatabaseValue::Int(2));
        {
            let mut a_mut = a.rc_clone();
            a_mut.add_child(b.rc_clone());
        }
        let result = structure.add_node(b.rc_clone());
        assert!(result.is_ok());
        assert_eq!(structure.nodes.len(), 2);
    }

    #[test]
    fn has_first_node_transitions_on_first_insert() {
        let mut un_strict: Structure<DatabaseValue> = Structure::new(None, "un-strict".to_string());
        assert!(!un_strict.has_first_node);
        let n: NodeRef<DatabaseValue> = NodeRef::new("n".to_string(), DatabaseValue::Int(1));
        un_strict.add_node(n).unwrap();
        assert!(un_strict.has_first_node);

        let mut semi_strict: Structure<DatabaseValue> =
            Structure::new(None, "semi-strict".to_string());
        assert!(!semi_strict.has_first_node);
        let m: NodeRef<DatabaseValue> = NodeRef::new("m".to_string(), DatabaseValue::Int(1));
        semi_strict.add_node(m).unwrap();
        assert!(semi_strict.has_first_node);

        // new() with a root already set should also mark has_first_node true
        let root: NodeRef<DatabaseValue> = NodeRef::new("root".to_string(), DatabaseValue::Int(1));
        let with_root: Structure<DatabaseValue> =
            Structure::new(Some(root), "un-strict".to_string());
        assert!(with_root.has_first_node);
    }

    #[test]
    fn delete_node_by_key_un_strict_allows_orphaning() {
        let root: NodeRef<DatabaseValue> = NodeRef::new("root".to_string(), DatabaseValue::Int(0));
        let mut structure: Structure<DatabaseValue> =
            Structure::new(Some(root.rc_clone()), "un-strict".to_string());

        let child: NodeRef<DatabaseValue> =
            NodeRef::new("child".to_string(), DatabaseValue::Int(1));
        {
            let mut root_mut = root.rc_clone();
            root_mut.add_child(child.rc_clone());
        }
        structure
            .add_node(child.rc_clone())
            .expect("child add failed");

        // deleting root would orphan child in a strict sense, but un-strict must allow it
        let deleted = structure.delete_node_by_key("root");
        assert!(deleted);
        assert!(structure.find_node_by_key("root").is_none());
        assert!(structure.find_node_by_key("child").is_some());
    }

    #[test]
    fn delete_node_by_key_semi_strict_blocks_orphaning_deletion() {
        let root: NodeRef<DatabaseValue> = NodeRef::new("root".to_string(), DatabaseValue::Int(0));
        let mut structure: Structure<DatabaseValue> =
            Structure::new(Some(root.rc_clone()), "semi-strict".to_string());

        let child: NodeRef<DatabaseValue> =
            NodeRef::new("child".to_string(), DatabaseValue::Int(1));
        {
            let mut root_mut = root.rc_clone();
            root_mut.add_child(child.rc_clone());
        }
        structure
            .add_node(child.rc_clone())
            .expect("child add failed");

        // deleting root would leave child with no valid parent/child in the structure -> must be blocked
        let deleted = structure.delete_node_by_key("root");
        assert!(!deleted);
        assert!(structure.find_node_by_key("root").is_some());
        assert!(structure.find_node_by_key("child").is_some());
    }

    #[test]
    fn delete_node_by_key_semi_strict_allows_safe_deletion() {
        let root: NodeRef<DatabaseValue> = NodeRef::new("root".to_string(), DatabaseValue::Int(0));
        let mut structure: Structure<DatabaseValue> =
            Structure::new(Some(root.rc_clone()), "semi-strict".to_string());

        let child_a: NodeRef<DatabaseValue> =
            NodeRef::new("child_a".to_string(), DatabaseValue::Int(1));
        let child_b: NodeRef<DatabaseValue> =
            NodeRef::new("child_b".to_string(), DatabaseValue::Int(2));
        {
            let mut root_mut = root.rc_clone();
            root_mut.add_child(child_a.rc_clone());
            root_mut.add_child(child_b.rc_clone());
        }
        structure
            .add_node(child_a.rc_clone())
            .expect("child_a add failed");
        structure
            .add_node(child_b.rc_clone())
            .expect("child_b add failed");

        // deleting child_a is safe: root still has child_b, and child_a has no children of its own
        let deleted = structure.delete_node_by_key("child_a");
        assert!(deleted);
        assert!(structure.find_node_by_key("child_a").is_none());
        assert!(structure.find_node_by_key("root").is_some());
        assert!(structure.find_node_by_key("child_b").is_some());
    }

    #[test]
    fn delete_only_node_in_semi_strict_structure_actually_removes_it() {
        let root = NodeRef::new("root".to_string(), DatabaseValue::Int(0));
        let mut structure = Structure::new(Some(root), "semi-strict".to_string());

        assert!(structure.delete_node_by_key("root"));
        assert!(structure.nodes.is_empty());
        assert!(structure.root.is_none());
        assert!(!structure.has_first_node);
    }

    #[test]
    fn semi_strict_deletion_accepts_neighbor_with_another_relation() {
        let root = NodeRef::new("root".to_string(), DatabaseValue::Int(0));
        let mut structure = Structure::new(Some(root.rc_clone()), "semi-strict".to_string());
        let middle = NodeRef::new("middle".to_string(), DatabaseValue::Int(1));
        let leaf = NodeRef::new("leaf".to_string(), DatabaseValue::Int(2));

        let mut root_mut = root.rc_clone();
        root_mut.add_child(middle.rc_clone());
        let mut middle_mut = middle.rc_clone();
        middle_mut.add_child(leaf.rc_clone());
        structure.add_node(middle).expect("middle add failed");
        structure.add_node(leaf).expect("leaf add failed");

        assert!(structure.delete_node_by_key("root"));
        assert!(structure.find_node_by_key("root").is_none());
        assert!(structure
            .find_node_by_key("middle")
            .unwrap()
            .has_child_by_key("leaf"));
    }

    #[test]
    fn duplicate_node_key_is_rejected_without_replacing_original() {
        let mut structure = Structure::new(None, "un-strict".to_string());
        structure
            .add_node(NodeRef::new("same".to_string(), DatabaseValue::Int(1)))
            .expect("first add failed");

        assert!(structure
            .add_node(NodeRef::new("same".to_string(), DatabaseValue::Int(2)))
            .is_err());
        assert_eq!(
            structure.find_node_by_key("same").unwrap().value(),
            DatabaseValue::Int(1)
        );
    }

    #[test]
    fn truncated_structure_returns_error() {
        let result = Structure::<DatabaseValue>::from_bytes(&[1, 2, 3]);
        assert!(result.is_err());
    }

    #[test]
    fn structure_with_missing_related_node_is_rejected() {
        let root = NodeRef::new("root".to_string(), DatabaseValue::Int(0));
        let external = NodeRef::new("external".to_string(), DatabaseValue::Int(1));
        let mut root_mut = root.rc_clone();
        root_mut.add_child(external);
        let structure = Structure::new(Some(root), "un-strict".to_string());

        let bytes = structure.to_bytes().expect("serialize failed");
        let result = Structure::<DatabaseValue>::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn add_edge_rejects_cycles_and_duplicates() {
        let mut structure = Structure::new(None, "un-strict".to_string());
        for key in ["a", "b", "c"] {
            structure
                .add_node(NodeRef::new(key.to_string(), DatabaseValue::Null))
                .expect("node add failed");
        }

        assert!(structure.add_edge_by_key("a", "b").is_ok());
        assert!(structure.add_edge_by_key("b", "c").is_ok());
        assert!(structure.add_edge_by_key("a", "b").is_err());
        assert!(structure.add_edge_by_key("c", "a").is_err());
        assert!(!structure
            .find_node_by_key("c")
            .unwrap()
            .has_child_by_key("a"));
    }

    #[test]
    fn persisted_cycle_is_rejected() {
        let a = NodeRef::new("a".to_string(), DatabaseValue::Null);
        let b = NodeRef::new("b".to_string(), DatabaseValue::Null);
        let mut a_mut = a.rc_clone();
        a_mut.add_child(b.rc_clone());
        let mut b_mut = b.rc_clone();
        b_mut.add_child(a.rc_clone());

        let mut structure = Structure::new(Some(a), "un-strict".to_string());
        structure
            .add_node(b)
            .expect("second node should be accepted before persistence validation");

        let bytes = structure.to_bytes().expect("serialize failed");
        let result = Structure::<DatabaseValue>::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn semi_strict_node_can_be_inserted_atomically_with_parent() {
        let mut structure = Structure::new(None, "semi-strict".to_string());
        structure
            .add_node(NodeRef::new("root".to_string(), DatabaseValue::Int(1)))
            .expect("first node add failed");

        structure
            .add_node_with_edges(
                NodeRef::new("child".to_string(), DatabaseValue::Int(2)),
                &["root".to_string()],
                &[],
            )
            .expect("atomic child add failed");

        assert!(structure
            .find_node_by_key("root")
            .unwrap()
            .has_child_by_key("child"));
        assert!(structure
            .find_node_by_key("child")
            .unwrap()
            .has_parent_by_key("root"));
    }

    #[test]
    fn failed_atomic_node_insert_does_not_mutate_structure() {
        let mut structure = Structure::new(None, "semi-strict".to_string());
        structure
            .add_node(NodeRef::new("root".to_string(), DatabaseValue::Int(1)))
            .expect("first node add failed");

        let error = structure
            .add_node_with_edges(
                NodeRef::new("child".to_string(), DatabaseValue::Int(2)),
                &["missing".to_string()],
                &[],
            )
            .expect_err("invalid insertion was accepted");

        assert_eq!(error, StructureError::MissingParent("missing".to_string()));
        assert_eq!(structure.len(), 1);
        assert!(structure.find_node_by_key("child").is_none());
    }
}
