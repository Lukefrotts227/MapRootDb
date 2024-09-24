use std::collections::HashMap;
use crate::node::NodeRef; // Update import to use NodeRef


pub struct Structure<T> {
    pub root: Option<NodeRef<T>>,           // Use NodeRef for root
    pub nodes: HashMap<String, NodeRef<T>>, // Use NodeRef in HashMap
    pub mode: String,
    pub has_first_node: bool,
}

impl<T> Structure<T> {
    pub fn new(root: Option<NodeRef<T>>, mode: String) -> Self {
        let mut nodes = HashMap::new();
        let mut has_first_node = false;

        if let Some(root) = &root {
            nodes.insert(root.key(), root.rc_clone()); // Insert root node
            has_first_node = true;
        }

        Structure {
            root,
            nodes,
            mode,
            has_first_node,
        }
    }

    // for the add node method we will have 3 modes strict, semi-strict , and un-strict, 
    // strict means that all parents and children must be present in the structure, unless the node is the first node in the structure
    // semi strict means at least one parent or child must be present in the structure, unless the node is the first node in the structure
    // un-strict means that the node can be added without any parents or children

    pub fn add_node(&mut self, node: NodeRef<T>) -> Result<NodeRef<T>, bool> {}

    pub fn strict_add(&mut self, node: NodeRef<T>) -> Result<NodeRef<T>, bool> {}

    pub fn semi_strict_add(&mut self, node: NodeRef<T>) -> Result<NodeRef<T>, bool> {}

    pub fn un_strict_add(&mut self, node: NodeRef<T>) -> Result<NodeRef<T>, bool> {}

    pub fn strict_test (&mut self, node: NodeRef<T>) -> bool {}

    pub fn semi_strict_test (&mut self, node: NodeRef<T>) -> bool  {
        // test if the node has at least one parent or child in the structure
        let parents = node.parents();
        let children = node.children();

        if parents.len() == 0 && children.len() == 0 && !self.has_first_node {
            return false
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



