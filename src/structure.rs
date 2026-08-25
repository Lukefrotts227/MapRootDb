use std::collections::HashMap;
use crate::node::NodeRef;
use serde::ser::Serialize;
use serde::de::DeserializeOwned;
use bincode::{serialize, deserialize};




pub struct Structure<T: Clone> {
    pub root: Option<NodeRef<T>>,           // Use NodeRef for root
    pub nodes: HashMap<String, NodeRef<T>>, // main hashmap for the structure that hashes to NodeRefs 
    pub mode: String,
    pub has_first_node: bool,
}

impl<T: Clone + Eq + Serialize> Structure<T> {
    pub fn new(root: Option<NodeRef<T>>, mode: String) -> Self {
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

    // for the add node method we will have 2 modes semi-strict and un-strict, 
    // semi strict means at least one parent or child must be present in the structure, unless the node is the first node in the structure
    // un-strict means that the node can be added without any parents or children
    // more modes will be added but this is good to get it going
    
   
    pub fn serialize_related_ids(&self) -> Vec<u8>{
        // in this function I am seraialing all the keys of the given hashmap so that I can rebuild by grabbing all the nodes by key allowing for rebuild
        let mut keys: Vec<String> = Vec::new(); 
        for (key, _) in self.nodes.iter(){
            keys.push(key.clone()); 
        }

        // seraialize with bincode
        let serialized = serialize(&keys).unwrap();

        // now lets retun this vector
        serialized 

    }

    pub fn serialize_related_nodes(&self) -> Vec<Vec<u8>>{
        // we can use the seralize function that I wrote for indiv nodes
        let mut over_vector: Vec<Vec<u8>> = Vec::new(); 

        // iterate through the map to get the vectors for each node
        for (_, node) in self.nodes.iter(){
            // clone the reference
            let n: NodeRef<T> = node.rc_clone(); 
            let serialized: Vec<u8> = n.serialize_node();
            over_vector.push(serialized);
        }
        over_vector

    }


    fn semi_strict_check_for_one(&self, node : NodeRef<T>, off_limit_key: &str) -> bool {
        

        let node_children: std::cell::Ref<'_, std::collections::HashSet<NodeRef<T>>> = node.children();
        let node_parents: std::cell::Ref<'_, std::collections::HashSet<NodeRef<T>>> = node.parents(); 
        if node_parents.len() == 1 && node_children.len() == 1 {
            return false
        }

        // iterate through node children looking for a valid node exit with true if found 
        // rembember to exclude the off limit key
        for child in node_children.iter() {
            // if the key is part of the structure and is not the off limit key return true
            if  child.key() != off_limit_key && self.nodes.contains_key(&child.key()) {
                return true
            }
        }
        // do the same for the parents
        for parent in node_parents.iter() {
            // if the key is part of the structure and is not the off limit key return true
            if  parent.key() != off_limit_key && self.nodes.contains_key(&parent.key()) {
                return true
            }
        }    
        // if all else fails then return false
        return false
        

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


        let prim_node = self.find_node_by_key(key);
        if prim_node.is_none() {
            return false
        }
        let mut node: NodeRef<T> = prim_node.unwrap(); 
        if self.mode == "un-strict" {
            self.nodes.remove(key);
            if self.root.is_some() && self.root.as_ref().unwrap().key() == key {
                self.root = None;
            }
            if self.nodes.len() == 0 {
                self.has_first_node = false;
            }
            node.delete_node();
            return true
        }

        // if the we are in a semi-strict db and the node is the last node then the removal is valid 
        let parents: std::cell::Ref<'_, std::collections::HashSet<NodeRef<T>>> = node.parents(); 
        let children: std::cell::Ref<'_, std::collections::HashSet<NodeRef<T>>> = node.children();
        
        if parents.len() == 0 && children.len() == 0 && !self.has_first_node {
            return false
        } else if parents.len() == 0 && children.len() == 0 && self.has_first_node {
            return true
        }


        // do the semi-strict test on the node being removed
        // start by iterating throught the parents and children of the node being removed and check for strictness without including the node being removed
        // if at any point the strictness is broken return false immediately

        // parent check 
        for parent in parents.iter() {
            // make sure the parent has at lease one valid child or parent not including the node being removed
            // use the semi_strict_check_for_one method to check for at least one valid parent or child
            if !self.semi_strict_check_for_one(parent.rc_clone(), key) {
                return false
            }
        }

        // child check
        for child in children.iter() {
            // make sure the child has at lease one valid child or parent not including the node being removed
            // use the semi_strict_check_for_one method to check for at least one valid parent or child
            if !self.semi_strict_check_for_one(child.rc_clone(), key) {
                return false
            }
        }

        // if the semi-stric test passes it is safe to remove the node from the structure
        self.nodes.remove(key);
        if self.root.is_some() && self.root.as_ref().unwrap().key() == key {
            self.root = None;
        }
        if self.nodes.len() == 0 {
            self.has_first_node = false;
        }
        // use borrow checker shenanigans to delete the node
        let mut other_same_node = node.rc_clone();
        other_same_node.delete_node();
        return true


    }
    pub fn remove_node_by_key(&mut self, key: &str) -> bool {
        // remove the node from the structure by key
        // only removes the node from the hashmap and does not actually delete the node 
        // all relationships will remain the same
        // return false if the node is not found
        // return false if this breaks the current strictness of the structure  

        let prim_node = self.find_node_by_key(key);
        if prim_node.is_none() {
            return false
        }

        let node: NodeRef<T> = prim_node.unwrap();
        if self.mode == "un-strict" {
            self.nodes.remove(key);
            if self.root.is_some() && self.root.as_ref().unwrap().key() == key {
                self.root = None;
            }
            if self.nodes.len() == 0 {
                self.has_first_node = false;
            }
            return true
        }

        let parents: std::cell::Ref<'_, std::collections::HashSet<NodeRef<T>>> = node.parents();
        let children: std::cell::Ref<'_, std::collections::HashSet<NodeRef<T>>> = node.children(); 

        if parents.len() == 0 && children.len() == 0 && !self.has_first_node {
            return false
        } else if parents.len() == 0 && children.len() == 0 && self.has_first_node {
            return true
        }


        for parent in parents.iter() {
            if !self.semi_strict_check_for_one(parent.rc_clone(), key) {
                return false
            }
        }

        for child in children.iter() {
            if !self.semi_strict_check_for_one(child.rc_clone(), key) {
                return false
            }
        }   

        self.nodes.remove(key); 
        if self.root.is_some() && self.root.as_ref().unwrap().key() == key {
            self.root = None;
        }
        if self.nodes.len() == 0 {
            self.has_first_node = false;
        }

        return true


    }

    pub fn find_node_by_key(&self, key: &str) -> Option<NodeRef<T>> {
        // find a node in the structure by key using the hashmap 
        // return the reference to the node if found
        // return None if not found 
        self.nodes.get(key).map(|node| node.rc_clone())

    }

    pub fn add_node(&mut self, node: NodeRef<T>) -> Result<NodeRef<T>, bool> {
        // depending on the mode use the correct add method
        match self.mode.as_str() {
            "semi-strict" => self.semi_strict_add(node),
            "un-strict" => self.un_strict_add(node),
            _ => Err(false),
        }
    }

    fn semi_strict_add(&mut self, node: NodeRef<T>) -> Result<NodeRef<T>, bool> {
        // perform a semi-strict test on the node to see if it can be added to the structure
        if self.semi_strict_test(node.rc_clone()) {
            self.nodes.insert(node.key(), node.rc_clone());
            self.has_first_node = true;
            return Ok(node)
        } else {
            return Err(false)
        }
    }

    fn un_strict_add(&mut self, node: NodeRef<T>) -> Result<NodeRef<T>, bool> {
        // simply add the node to the structure
        self.nodes.insert(node.key(), node.rc_clone());
        self.has_first_node = true; 
        return Ok(node)
    }

    pub fn semi_strict_test (&mut self, node: NodeRef<T>) -> bool  {
        // test if the node has at least one parent or child in the structure
        let parents = node.parents();
        let children = node.children();

        if parents.len() == 0 && children.len() == 0 && !self.has_first_node {
            return false
        } else if parents.len() == 0 && children.len() == 0 && self.has_first_node {
            return true
        }

        // iterate through the parents hashset and if the parent is in the structure return true

        for parent in parents.iter() {
            if self.nodes.contains_key(&parent.key()) {
                return true
            }           
        }

        // iterate through the children hashset and if the child is in the structure return true
        for child in children.iter() {
            if self.nodes.contains_key(&child.key()) {
                return true
            }
        }
        
        // if nothing has been found return false
        return false
    }


}

impl<T: Clone + Eq + Serialize + DeserializeOwned> Structure<T> {
    // Rebuild a Structure from the raw serialized node blobs produced by serialize_related_nodes.
    pub fn from_serialized_nodes(serialized_nodes: Vec<Vec<u8>>, root_key: Option<String>, mode: String) -> Self {
        let mut node_map: HashMap<String, NodeRef<T>> = HashMap::new();
        let mut child_edges: Vec<(String, Vec<String>)> = Vec::new();

        for node_bytes in serialized_nodes {
            let (node_ref, _parent_keys, child_keys) = NodeRef::deserialize_node(&node_bytes);
            let key = node_ref.key();
            node_map.insert(key.clone(), node_ref);
            child_edges.push((key, child_keys));
        }

        // Wire up edges — add_child sets both sides, so only process children to avoid double-linking.
        for (node_key, child_keys) in child_edges {
            let mut node = node_map[&node_key].rc_clone();
            for child_key in child_keys {
                if let Some(child) = node_map.get(&child_key) {
                    node.add_child(child.rc_clone());
                }
            }
        }

        let root = root_key.and_then(|k| node_map.get(&k).map(|n| n.rc_clone()));
        let has_first_node = !node_map.is_empty();
        Structure { root, nodes: node_map, mode, has_first_node }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        fn write_framed(buf: &mut Vec<u8>, data: &[u8]) {
            buf.extend_from_slice(&(data.len() as u64).to_le_bytes());
            buf.extend_from_slice(data);
        }

        let mut buf = Vec::new();
        write_framed(&mut buf, &serialize(&self.root.as_ref().map(|r| r.key())).unwrap());
        write_framed(&mut buf, &serialize(&self.mode).unwrap());

        let node_blobs = self.serialize_related_nodes();
        buf.extend_from_slice(&(node_blobs.len() as u64).to_le_bytes());
        for blob in node_blobs {
            write_framed(&mut buf, &blob);
        }

        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        fn read_framed<'a>(bytes: &'a [u8], offset: &mut usize) -> &'a [u8] {
            let len = u64::from_le_bytes(bytes[*offset..*offset + 8].try_into().unwrap()) as usize;
            *offset += 8;
            let data = &bytes[*offset..*offset + len];
            *offset += len;
            data
        }

        let mut offset = 0;
        let root_key: Option<String> = deserialize(read_framed(bytes, &mut offset)).unwrap();
        let mode: String = deserialize(read_framed(bytes, &mut offset)).unwrap();
        let node_count = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize;
        offset += 8;

        let mut serialized_nodes = Vec::new();
        for _ in 0..node_count {
            serialized_nodes.push(read_framed(bytes, &mut offset).to_vec());
        }

        Self::from_serialized_nodes(serialized_nodes, root_key, mode)
    }

    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        std::fs::write(path, self.to_bytes())
    }

    pub fn load_from_file(path: &str) -> std::io::Result<Self> {
        Ok(Self::from_bytes(&std::fs::read(path)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DatabaseValue;
    use crate::node::NodeRef;

    fn scratch_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("maprootdb_test_{}_{}.bin", name, std::process::id()));
        p
    }

    #[test]
    fn round_trip_empty_structure() {
        let structure: Structure<DatabaseValue> = Structure::new(None, "un-strict".to_string());

        let bytes = structure.to_bytes();
        let restored: Structure<DatabaseValue> = Structure::from_bytes(&bytes);

        assert!(restored.root.is_none());
        assert_eq!(restored.nodes.len(), 0);
        assert_eq!(restored.mode, "un-strict");
        assert_eq!(restored.has_first_node, false);
    }

    #[test]
    fn round_trip_single_node() {
        let root: NodeRef<DatabaseValue> = NodeRef::new("root".to_string(), DatabaseValue::Int(42));
        let structure: Structure<DatabaseValue> = Structure::new(Some(root), "un-strict".to_string());

        let bytes = structure.to_bytes();
        let restored: Structure<DatabaseValue> = Structure::from_bytes(&bytes);

        assert_eq!(restored.nodes.len(), 1);
        assert_eq!(restored.mode, "un-strict");
        assert!(restored.has_first_node);
        assert!(restored.root.is_some());
        assert_eq!(restored.root.as_ref().unwrap().key(), "root");
        assert_eq!(restored.root.as_ref().unwrap().value(), DatabaseValue::Int(42));
    }

    #[test]
    fn round_trip_multi_node_with_edges() {
        let root: NodeRef<DatabaseValue> = NodeRef::new("root".to_string(), DatabaseValue::Text("root-val".to_string()));
        let mut structure: Structure<DatabaseValue> = Structure::new(Some(root.rc_clone()), "semi-strict".to_string());

        let mut child_a: NodeRef<DatabaseValue> = NodeRef::new("child_a".to_string(), DatabaseValue::Int(1));
        let child_b: NodeRef<DatabaseValue> = NodeRef::new("child_b".to_string(), DatabaseValue::Bool(true));
        let grandchild: NodeRef<DatabaseValue> = NodeRef::new("grandchild".to_string(), DatabaseValue::Float(3.14));

        // root -> child_a, root -> child_b, child_a -> grandchild
        {
            let mut root_mut = root.rc_clone();
            root_mut.add_child(child_a.rc_clone());
            root_mut.add_child(child_b.rc_clone());
        }
        child_a.add_child(grandchild.rc_clone());

        structure.add_node(child_a.rc_clone()).expect("child_a add failed");
        structure.add_node(child_b.rc_clone()).expect("child_b add failed");
        structure.add_node(grandchild.rc_clone()).expect("grandchild add failed");

        let bytes = structure.to_bytes();
        let restored: Structure<DatabaseValue> = Structure::from_bytes(&bytes);

        assert_eq!(restored.mode, "semi-strict");
        assert!(restored.has_first_node);
        assert_eq!(restored.nodes.len(), 4);
        assert_eq!(restored.root.as_ref().unwrap().key(), "root");

        // check values
        assert_eq!(restored.find_node_by_key("root").unwrap().value(), DatabaseValue::Text("root-val".to_string()));
        assert_eq!(restored.find_node_by_key("child_a").unwrap().value(), DatabaseValue::Int(1));
        assert_eq!(restored.find_node_by_key("child_b").unwrap().value(), DatabaseValue::Bool(true));
        assert_eq!(restored.find_node_by_key("grandchild").unwrap().value(), DatabaseValue::Float(3.14));

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
        let mut structure: Structure<DatabaseValue> = Structure::new(Some(root.rc_clone()), "semi-strict".to_string());

        let child: NodeRef<DatabaseValue> = NodeRef::new("child".to_string(), DatabaseValue::Int(7));
        {
            let mut root_mut = root.rc_clone();
            root_mut.add_child(child.rc_clone());
        }
        structure.add_node(child.rc_clone()).expect("child add failed");

        let path = scratch_path("structure_file_roundtrip");
        let path_str = path.to_str().unwrap();

        structure.save_to_file(path_str).expect("save_to_file failed");
        let restored: Structure<DatabaseValue> = Structure::load_from_file(path_str).expect("load_from_file failed");

        std::fs::remove_file(&path).ok();

        assert_eq!(restored.mode, "semi-strict");
        assert_eq!(restored.nodes.len(), 2);
        assert_eq!(restored.root.as_ref().unwrap().key(), "root");
        assert!(restored.find_node_by_key("root").unwrap().has_child_by_key("child"));
        assert!(restored.find_node_by_key("child").unwrap().has_parent_by_key("root"));
    }
}
