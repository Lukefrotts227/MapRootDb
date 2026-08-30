use bincode::{deserialize, serialize};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io;
use std::rc::Rc;

#[derive(Clone)]
/// A cloneable reference to a graph node.
///
/// Clones point to the same interior-mutable node. `NodeRef` is intentionally not
/// thread-safe; the server keeps the graph on one owner thread.
pub struct NodeRef<T: Clone>(Rc<RefCell<Node<T>>>);

impl<T: Clone> fmt::Debug for NodeRef<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.try_borrow() {
            Ok(node) => formatter
                .debug_struct("NodeRef")
                .field("key", &node.key)
                .finish(),
            Err(_) => formatter
                .debug_struct("NodeRef")
                .field("state", &"currently mutably borrowed")
                .finish(),
        }
    }
}

impl<T: Clone + Serialize> NodeRef<T> {
    pub(crate) fn serialize_node(&self) -> io::Result<Vec<u8>> {
        fn serialization_error(error: bincode::Error) -> io::Error {
            io::Error::new(io::ErrorKind::InvalidData, error)
        }

        let node: Ref<'_, Node<T>> = self.borrow();
        // split the node into its components to serialize each
        let key: String = node.key.clone();
        let value: T = node.value.clone();
        let mut parents: Vec<String> = node
            .parents
            .iter()
            .map(|parent| parent.key())
            .collect::<Vec<String>>();
        let mut children: Vec<String> = node
            .children
            .iter()
            .map(|child| child.key())
            .collect::<Vec<String>>();
        parents.sort();
        children.sort();

        // now we serialize each component
        let key_serialized: Vec<u8> = serialize(&key).map_err(serialization_error)?;
        let value_serialized: Vec<u8> = serialize(&value).map_err(serialization_error)?;
        let parents_serialized: Vec<u8> = serialize(&parents).map_err(serialization_error)?;
        let children_serialized: Vec<u8> = serialize(&children).map_err(serialization_error)?;

        fn write_with_length(buffer: &mut Vec<u8>, data: Vec<u8>) {
            let len = data.len() as u64;
            buffer.extend_from_slice(&len.to_le_bytes());
            buffer.extend_from_slice(&data);
        }

        let mut s_node = Vec::new();

        write_with_length(&mut s_node, key_serialized);
        write_with_length(&mut s_node, value_serialized);
        write_with_length(&mut s_node, parents_serialized);
        write_with_length(&mut s_node, children_serialized);

        Ok(s_node)
    }

    /// Creates an unconnected node with the supplied key and value.
    pub fn new(key: String, value: T) -> NodeRef<T> {
        NodeRef(Rc::new(RefCell::new(Node {
            key,
            value,
            parents: HashSet::new(),
            children: HashSet::new(),
        })))
    }

    /// Returns a copy of this node's key.
    pub fn key(&self) -> String {
        self.0.borrow().key.clone()
    }

    /// Returns another reference to the same node.
    pub fn rc_clone(&self) -> NodeRef<T> {
        let rc: Rc<RefCell<Node<T>>> = Rc::clone(&self.0);
        NodeRef(rc)
    }

    fn borrow(&self) -> Ref<'_, Node<T>> {
        self.0.borrow()
    }

    /// Returns a clone of this node's value.
    pub fn value(&self) -> T {
        self.0.borrow().value.clone()
    }

    pub(crate) fn add_child(&mut self, child: NodeRef<T>) {
        if Rc::ptr_eq(&self.0, &child.0) {
            return;
        }

        {
            let mut node_self: std::cell::RefMut<'_, Node<T>> = RefCell::borrow_mut(&self.0);
            node_self.children.insert(child.rc_clone());
        } // `node_self` is dropped here

        {
            let mut node_child: std::cell::RefMut<'_, Node<T>> = RefCell::borrow_mut(&child.0);
            node_child.parents.insert(self.rc_clone());
        }
    }

    /// Borrows the set of parent nodes.
    pub fn parents(&self) -> Ref<'_, HashSet<NodeRef<T>>> {
        Ref::map(self.0.borrow(), |node| &node.parents)
    }

    /// Borrows the set of child nodes.
    pub fn children(&self) -> Ref<'_, HashSet<NodeRef<T>>> {
        Ref::map(self.0.borrow(), |node| &node.children)
    }

    /// Reports whether this node has the named parent.
    pub fn has_parent_by_key(&self, key: &str) -> bool {
        self.parents().iter().any(|parent| parent.key() == key)
    }

    /// Reports whether this node has the named child.
    pub fn has_child_by_key(&self, key: &str) -> bool {
        self.children().iter().any(|child| child.key() == key)
    }

    /// Returns the named parent, if present.
    pub fn get_parent_by_key(&self, key: &str) -> Option<NodeRef<T>> {
        self.parents()
            .iter()
            .find(|parent| parent.key() == key)
            .map(|parent| parent.rc_clone())
    }

    /// Returns the named child, if present.
    pub fn get_child_by_key(&self, key: &str) -> Option<NodeRef<T>> {
        self.children()
            .iter()
            .find(|child| child.key() == key)
            .map(|child| child.rc_clone())
    }

    /// Replaces the node's value.
    pub fn edit_value(&mut self, value: T) {
        let mut node: std::cell::RefMut<'_, Node<T>> = RefCell::borrow_mut(&self.0);
        node.value = value;
    }

    pub(crate) fn delete_node(&mut self) {
        // remove the node from the given sets of all its parents and children
        // perma delete the node after this
        let node: Ref<'_, Node<T>> = RefCell::borrow(&self.0);

        // remove the node from all its parents
        for parent in node.parents.iter() {
            let mut parent_node: RefMut<'_, Node<T>> = RefCell::borrow_mut(&parent.0);
            parent_node.children.remove(&self.rc_clone());
        }

        // remove the node from all its children
        for child in node.children.iter() {
            let mut child_node: RefMut<'_, Node<T>> = RefCell::borrow_mut(&child.0);
            child_node.parents.remove(&self.rc_clone());
        }

        // delete the node
        drop(node);
    }
}

impl<T: Clone> Hash for NodeRef<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.borrow().key.hash(state);
    }
}

impl<T: Clone> PartialEq for NodeRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.borrow().key == other.0.borrow().key
    }
}

impl<T: Clone> Eq for NodeRef<T> {}
pub(crate) struct Node<T: Clone> {
    key: String,
    value: T,
    parents: HashSet<NodeRef<T>>,
    children: HashSet<NodeRef<T>>,
}

impl<T: Clone + Serialize + DeserializeOwned> NodeRef<T> {
    // Returns (node, parent_keys, child_keys) — caller wires up edges after all nodes are created.
    pub(crate) fn deserialize_node(
        bytes: &[u8],
    ) -> io::Result<(NodeRef<T>, Vec<String>, Vec<String>)> {
        fn invalid_data(message: impl Into<String>) -> io::Error {
            io::Error::new(io::ErrorKind::InvalidData, message.into())
        }

        fn read_field<'a>(bytes: &'a [u8], offset: &mut usize) -> io::Result<&'a [u8]> {
            let length_end = offset
                .checked_add(8)
                .ok_or_else(|| invalid_data("node field length overflow"))?;
            let length_bytes = bytes
                .get(*offset..length_end)
                .ok_or_else(|| invalid_data("truncated node field length"))?;
            let length = u64::from_le_bytes(
                length_bytes
                    .try_into()
                    .map_err(|_| invalid_data("invalid node field length"))?,
            );
            let length = usize::try_from(length)
                .map_err(|_| invalid_data("node field is too large for this platform"))?;
            let data_end = length_end
                .checked_add(length)
                .ok_or_else(|| invalid_data("node field length overflow"))?;
            let data = bytes
                .get(length_end..data_end)
                .ok_or_else(|| invalid_data("truncated node field"))?;
            *offset = data_end;
            Ok(data)
        }

        let mut offset = 0;
        let key: String = deserialize(read_field(bytes, &mut offset)?)
            .map_err(|error| invalid_data(format!("invalid node key: {error}")))?;
        let value: T = deserialize(read_field(bytes, &mut offset)?)
            .map_err(|error| invalid_data(format!("invalid node value: {error}")))?;
        let parent_keys: Vec<String> = deserialize(read_field(bytes, &mut offset)?)
            .map_err(|error| invalid_data(format!("invalid node parents: {error}")))?;
        let child_keys: Vec<String> = deserialize(read_field(bytes, &mut offset)?)
            .map_err(|error| invalid_data(format!("invalid node children: {error}")))?;

        if offset != bytes.len() {
            return Err(invalid_data("trailing bytes after node"));
        }

        Ok((NodeRef::new(key, value), parent_keys, child_keys))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_child_is_bidirectional() {
        let mut a: NodeRef<String> = NodeRef::new("a".to_string(), "va".to_string());
        let b: NodeRef<String> = NodeRef::new("b".to_string(), "vb".to_string());

        a.add_child(b.rc_clone());
        assert!(a.has_child_by_key("b"));
        assert!(b.has_parent_by_key("a"));
    }

    #[test]
    fn delete_node_removes_from_parents_and_children() {
        let mut parent: NodeRef<String> = NodeRef::new("parent".to_string(), "p".to_string());
        let mut child: NodeRef<String> = NodeRef::new("child".to_string(), "c".to_string());

        parent.add_child(child.rc_clone());
        assert!(parent.has_child_by_key("child"));
        assert!(child.has_parent_by_key("parent"));

        child.delete_node();

        assert!(!parent.has_child_by_key("child"));
    }

    #[test]
    fn has_parent_by_key_and_get_child_by_key_work() {
        let mut a: NodeRef<String> = NodeRef::new("a".to_string(), "va".to_string());
        let b: NodeRef<String> = NodeRef::new("b".to_string(), "vb".to_string());

        a.add_child(b.rc_clone());

        assert!(!a.has_parent_by_key("b"));
        assert!(b.has_parent_by_key("a"));

        let found = a.get_child_by_key("b");
        assert!(found.is_some());
        assert_eq!(found.unwrap().key(), "b");

        assert!(a.get_child_by_key("nonexistent").is_none());
    }

    #[test]
    fn edit_value_updates_value() {
        let mut a: NodeRef<String> = NodeRef::new("a".to_string(), "old".to_string());
        assert_eq!(a.value(), "old".to_string());

        a.edit_value("new".to_string());
        assert_eq!(a.value(), "new".to_string());
    }

    #[test]
    fn serialize_deserialize_node_round_trips() {
        let mut node: NodeRef<String> = NodeRef::new("key1".to_string(), "value1".to_string());
        let mut parent: NodeRef<String> = NodeRef::new("parent1".to_string(), "pval".to_string());
        let child: NodeRef<String> = NodeRef::new("child1".to_string(), "cval".to_string());

        parent.add_child(node.rc_clone());
        node.add_child(child.rc_clone());

        let bytes = node.serialize_node().expect("serialize failed");
        let (deserialized, parent_keys, child_keys) =
            NodeRef::<String>::deserialize_node(&bytes).expect("deserialize failed");

        assert_eq!(deserialized.key(), "key1");
        assert_eq!(deserialized.value(), "value1".to_string());
        assert_eq!(parent_keys, vec!["parent1".to_string()]);
        assert_eq!(child_keys, vec!["child1".to_string()]);
    }

    #[test]
    fn truncated_node_returns_error() {
        let result = NodeRef::<String>::deserialize_node(&[1, 2, 3]);
        assert!(result.is_err());
    }

    #[test]
    fn self_edges_are_ignored_without_panicking() {
        let mut node = NodeRef::new("node".to_string(), "value".to_string());
        node.add_child(node.rc_clone());

        assert!(node.children().is_empty());
        assert!(node.parents().is_empty());
    }
}
