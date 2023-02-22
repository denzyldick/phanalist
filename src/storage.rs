use std::{collections::HashMap, fmt::Debug};

use crate::rules::File;
use rocksdb::{DBCommon, Options, SingleThreaded, DB};
use serde::{Deserialize, Serialize};

pub fn put<T: Serialize + Debug>(db: &DB, key: String, file: T) {
    let bytes = match serde_json::to_string(&file) {
        Ok(o) => {
            match db.put(key, o) {
                Err(e) => {
                    // println!("Helloworld");
                    // println!("{file:?}");
                    // println!("{e}");
                }
                Ok(ok) => {}
            };
        }
        Err(e) => {
            println!("{file:#?}");
            print!("{e}");
        }
    };
}

pub fn get(db: &DB, key: String) -> Option<File> {
    let path = "/tmp";

    match db.get(key) {
        Ok(Some(f)) => {
            let file = serde_json::from_slice(&f).unwrap();
            Some(file)
        }

        Err(e) => {
            println!("{e}");
            None
        }
        _ => None,
    }
}
