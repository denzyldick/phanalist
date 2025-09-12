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
        if (self.mapping.contains_key(&namespace) == false) {
            self.mapping.insert(namespace, file);
        }
    }

    fn mapping(&self) -> &HashMap<String, File> {
        &self.mapping
    }

    fn get_method_parameters() {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::File;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn test_add_to_index() {
        let indexer = Indexer::new(HashMap::new());
        let file = File::new(PathBuf::new(), String::from("<?php"));
        indexer.add(String::from("\\Namespace\\Test"), file);

        let parameters = indexer.get_method_parameters();
    }
}
