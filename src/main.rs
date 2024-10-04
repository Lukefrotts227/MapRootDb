mod node;
mod structure;
mod database;

use node::NodeRef;
use structure::Structure;

fn main() {
    // create a string structure for testing
    let mut structure: Structure<String> = Structure::new(None, "un-strict".to_string());

    // this structure will not contain a root 

    let mut first_node: NodeRef<String> = NodeRef::new("1".to_string(), "Bob".to_string()); 
    let mut second_node: NodeRef<String> = NodeRef::new("2".to_string(), "Alice".to_string());   
    let mut third_node: NodeRef<String> = NodeRef::new("3".to_string(), "Charlie".to_string());  
    let mut fourth_node: NodeRef<String> = NodeRef::new("4".to_string(), "David".to_string());   
    let mut fifth_node: NodeRef<String> = NodeRef::new("5".to_string(), "Eve".to_string()); 

    // create some relationships now
    first_node.add_child(third_node.rc_clone());

    second_node.add_parent(fourth_node.rc_clone()); 

    first_node.add_parent(fifth_node.rc_clone()); 

    third_node.add_child(second_node.rc_clone());

    // add it all to the structure now 
    structure.add_node(first_node.rc_clone());
    structure.add_node(second_node.rc_clone()); 
    structure.add_node(third_node.rc_clone()); 
    structure.add_node(fourth_node.rc_clone()); 

    let s1_vector: Vec<u8> = structure.serialize_related_ids(); 
    let s2_vector: Vec<Vec<u8>> = structure.serialize_related_nodes(); 

    // print out for testing 
    println!("Serialized related ids: {:?}", s1_vector);
    println!("Serialized related nodes: {:?}", s2_vector);



}