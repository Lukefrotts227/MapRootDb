mod node;
mod structure;

use node::NodeRef;
use structure::Structure;

fn main() {
    // Initialize the structure without a root node
    let mut structure = Structure::new(None, "semi-strict".to_string());

    // Add a new node to the structure
    let mut node: NodeRef<i32> = NodeRef::new("node1".to_string(), 42);
    let other_node: NodeRef<i32> = NodeRef::new("node2".to_string(), 451); 
    node.add_child(other_node.rc_clone());
    structure.nodes.insert(node.key(), node.rc_clone());

    // Print information about the structure
    println!("Structure has first node: {}", structure.has_first_node);
    println!("Number of nodes in the structure: {}", structure.nodes.len());
    println!("Node one has the child node: {}"); 

    // Verify that the node was added correctly
    if let Some(added_node) = structure.nodes.get("node1") {
        println!("Added node key: {}", added_node.key());
    }
}
