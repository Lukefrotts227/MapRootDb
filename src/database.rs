// this is the structure that uses the structure module to build out the database.

use crate::structure::{self, Structure}; 
use crate::node::NodeRef; 
use std::collections::HashMap;  
use prost::Message;
 

pub struct PrimInitDatabase<T: Clone>{
    pub data: Vec<PrimInitStructureWrapper<T>>, 

}

impl<T: Clone> PrimInitDatabase<T>{
    pub fn new() -> Self {
        PrimInitDatabase { data: Vec::new()}
    }
    pub fn add_structure(&mut self, structure: PrimInitStructureWrapper<T>){
        self.data.push(structure); 
    }

    // return a given structure in a mutable form 
    pub fn structure_mut(&mut self, name: &str) -> Option<&mut PrimInitStructureWrapper<T>>{
        for s in &mut self.data{
            if s.name == name{
                let mut f = s; 
                return Some(f); 
            }
        }
        None
    }
    
    // do it in read-only or not mutable form 
    pub fn structure(&self, name: &str) -> Option<&PrimInitStructureWrapper<T>>{
        for s in &self.data{
            if s.name == name{
                return Some(s); 
            }
        }
        None
    }
    
}

pub struct PrimInitStructureWrapper<T: Clone>{
    pub name: String, // the name of the structure in the db
    pub structure: Structure<T>, 


}
impl<T: Clone> PrimInitStructureWrapper<T>{
    pub fn new(name: String, structure: Structure<T>) -> Self{
        PrimInitStructureWrapper { name, structure}
    }
}