use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, Clone, Default)]
pub struct Data {
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub id: u128,
}

impl Data {
    pub fn new(raw_data: Vec<u8>) -> Self {
        let id: u128 = get_id(&raw_data);
        Data { data: raw_data, id }
    }
}

fn get_id(raw_data: &[u8]) -> u128 {
    let mut hasher = DefaultHasher::new();
    raw_data.hash(&mut hasher);
    hasher.finish().into()
}

impl PartialEq for Data {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
