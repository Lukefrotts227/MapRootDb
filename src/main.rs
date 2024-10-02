mod node;
mod structure;
mod database;

use node::NodeRef;
use structure::Structure;

fn main() {
    // create a string structure for testing
    let mut structure: Structure<String> = Structure::new(None, "un-strict".to_string());

    // this structure will not contain a root 

    let first_node = NodeRef::new("1".to_string(), "Bob"); 
    let second_node = NodeRef::new("2".to_string(), "Alice");   
    let third_node = NodeRef::new("3".to_string(), "Charlie");  
    let fourth_node = NodeRef::new("4".to_string(), "David");   
    let fifth_node = NodeRef::new("5".to_string(), "Eve"); 


    

}