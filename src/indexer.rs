use crate::file::File;
use std::collections::HashMap;
struct Indexer {
    mapping: HashMap<String, File>,
}

impl Indexer {
    fn new(mapping: HashMap<String, File>) -> Self {
        Self { mapping }
    }

    fn add(mut self, namespace: String, file: File) {
       if(self.mapping.contains_key(&namespace) == false){
          self.mapping.insert(namespace, file);
        }
    }

    fn mapping(&self) -> &HashMap<String, File> {
        &self.mapping
    }
}


#[cfg(test)]
mod tests{
    use super::*;
    use crate::file::File;
    use std::collections::HashMap;
    
    #[test]
    fn test_add_to_index(){

}
    
    
}
