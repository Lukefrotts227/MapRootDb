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

    // for the add node method we will have 2 modes semi-strict and un-strict, 
    // semi strict means at least one parent or child must be present in the structure, unless the node is the first node in the structure
    // un-strict means that the node can be added without any parents or children
    // more modes will be added but this is good to get it going

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
            return Ok(node)
        } else {
            return Err(false)
        }
    }

    fn un_strict_add(&mut self, node: NodeRef<T>) -> Result<NodeRef<T>, bool> {
        // simply add the node to the structure
        self.nodes.insert(node.key(), node.rc_clone());
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



