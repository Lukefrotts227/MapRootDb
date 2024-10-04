use std::rc::Rc;
use std::cell::{RefCell, Ref, RefMut};
use std::collections::HashSet; 
use std::hash::{Hash, Hasher}; 
use bincode::{serialize, deserialize};
use serde::{Serialize, Deserialize};
use std::io::Write;




#[derive(Clone)]
pub struct NodeRef<T: Clone>(Rc<RefCell<Node<T>>>);




impl<T: Clone + Serialize> NodeRef<T> {

    pub fn serialize_node(&self) -> Vec<u8> {
        let node: Ref<'_, Node<T>>= self.borrow(); 
        // split the node into its components to serialize each 
        let key: String = node.key.clone();
        let value: T = node.value.clone();
        let parents: Vec<String> = node.parents.iter().map(|parent| parent.key()).collect::<Vec<String>>();
        let children: Vec<String> = node.children.iter().map(|child| child.key()).collect::<Vec<String>>();

        // now we serialize each component
        let key_serialized: Vec<u8> = serialize(&key).unwrap();
        let value_serialized: Vec<u8> = serialize(&value).unwrap();
        let parents_serialized: Vec<u8> = serialize(&parents).unwrap();
        let children_serialized: Vec<u8> = serialize(&children).unwrap();

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

        s_node

        
    } 

    pub fn new(key: String, value: T) -> NodeRef<T> {
        NodeRef(Rc::new(RefCell::new(Node {
            key,
            value,
            parents: HashSet::new(),
            children: HashSet::new(),
        })))
    }

    pub fn key(&self) -> String {
        self.0.borrow().key.clone()
    }
    
    pub fn rc_clone(&self) -> NodeRef<T> {
        let rc: Rc<RefCell<Node<T>>> = Rc::clone(&self.0);
        NodeRef(rc)
    }

    pub fn borrow(&self) -> Ref<'_, Node<T>> {
        self.0.borrow()
    }

    pub fn borrow_mut(&self) -> RefMut<'_, Node<T>> {
        self.0.borrow_mut()
    }

    pub fn value(&self) -> T {
        self.0.borrow().value.clone()
    }    

    pub fn add_parent(&mut self, parent: NodeRef<T>) {
        // Add the parent to the current node's parent set
        {
            let mut node_self: std::cell::RefMut<'_, Node<T>> = RefCell::borrow_mut(&self.0); 
            node_self.parents.insert(parent.rc_clone());
        }
        {
            let mut node_parent: std::cell::RefMut<'_, Node<T>> = RefCell::borrow_mut(&parent.0);
            node_parent.children.insert(self.rc_clone()); 
        }
    }

    pub fn add_child(&mut self, child: NodeRef<T>) {
        {
            let mut node_self: std::cell::RefMut<'_, Node<T>> = RefCell::borrow_mut(&self.0);
            node_self.children.insert(child.rc_clone());
        } // `node_self` is dropped here
    
        {
            let mut node_child: std::cell::RefMut<'_, Node<T>> = RefCell::borrow_mut(&child.0); 
            node_child.parents.insert(self.rc_clone());
        }

    }

    pub fn parents(&self) -> Ref<'_, HashSet<NodeRef<T>>> {
        Ref::map(self.0.borrow(), |node| &node.parents)
    }

    // Returns an immutable reference to the set of children
    pub fn children(&self) -> Ref<'_, HashSet<NodeRef<T>>> {
        Ref::map(self.0.borrow(), |node| &node.children)
    } 

    pub fn has_parent_by_key(&self, key: &str) -> bool {
        self.parents().iter().any(|parent| parent.key() == key)
    }

    pub fn has_child_by_key(&self, key: &str) -> bool {
        self.children().iter().any(|child| child.key() == key)
    }

    pub fn get_parent_by_key(&self, key: &str) -> Option<NodeRef<T>> {
        self.parents().iter().find(|parent| parent.key() == key).map(|parent| parent.rc_clone())
    }

    pub fn get_child_by_key(&self, key: &str) -> Option<NodeRef<T>> {
        self.children().iter().find(|child| child.key() == key).map(|child| child.rc_clone())
    }

    pub fn edit_value(&mut self, value: T) {
        let mut node: std::cell::RefMut<'_, Node<T>> = RefCell::borrow_mut(&self.0);
        node.value = value;
    }

    pub fn delete_node(&mut self) {
        // remove the node from the given sets of all its parents and children
        // perma delete the node after this
        let mut node: std::cell::RefMut<'_, Node<T>> = RefCell::borrow_mut(&self.0);
        

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
        self.0.borrow_mut().key.hash(state);
    }
}

impl<T: Clone> PartialEq for NodeRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.borrow_mut().key == other.0.borrow().key
    }
}

impl<T: Clone> Eq for NodeRef<T> {}
pub struct Node<T: Clone> {
    pub key: String,
    pub value: T,
    pub parents: HashSet<NodeRef<T>>,
    pub children: HashSet<NodeRef<T>>,
}

impl<T: Clone + Serialize> Node<T> {
    pub fn new(key: String, value: T) -> NodeRef<T> {
        NodeRef::new(key, value)
    }
}

