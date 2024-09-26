// this is the structure that uses the structure module to build out the database\\

use crate::structure::Structure; 
use crate::node::NodeRef; 


pub struct prim_init_database<T: Clone>{
    pub data: Vec<Structure<T>>, 

}

impl<T: Clone> prim_init_database<T>{

}

pub struct prim_init_structure_wrapper<T: Clone>{
    pub structure: Structure<T>, 


}
impl<T: Clone> prim_init_structure_wrapper<T>{}
