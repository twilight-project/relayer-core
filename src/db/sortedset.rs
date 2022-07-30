#![allow(dead_code)]
#![allow(unused_imports)]
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SortedSet {
    pub sorted_order: Vec<(Uuid, i64)>,
    pub hash: HashSet<Uuid>,
    pub len: usize,
    pub min_price: i64,
    pub max_price: i64,
}
impl SortedSet {
    pub fn new() -> Self {
        SortedSet {
            sorted_order: Vec::new(),
            hash: HashSet::new(),
            len: 0,
            min_price: 0,
            max_price: 0,
        }
    }

    pub fn add(&mut self, uuid: Uuid, entry_price_i64: i64) -> Result<(), std::io::Error> {
        if self.hash.insert(uuid) {
            self.sorted_order.push((uuid, entry_price_i64));
            self.len += 1;
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Key already exist",
            ))
        }
    }

    pub fn remove(&mut self, uuid: Uuid) -> Result<(Uuid, i64), std::io::Error> {
        let mut value: Vec<(Uuid, i64)> = Vec::new();

        if self.hash.remove(&uuid) {
            let key_index = self.sorted_order.iter().position(|&(x, _y)| x == uuid);
            if key_index.is_some() {
                value = self
                    .sorted_order
                    .drain(key_index.unwrap()..(key_index.unwrap() + 1))
                    .collect();
                self.len -= 1;
            }
        } else {
        }
        match value.pop() {
            Some((id, price)) => Ok((id, price)),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Key does not exist",
            )),
        }
    }

    pub fn sort(&mut self) {
        self.sorted_order.sort_by_key(|p| p.1);
        self.min_price = self.sorted_order[0].1;
        self.max_price = self.sorted_order[self.len - 1].1;
    }

    pub fn read(&mut self) -> SortedSet {
        self.sort();
        let result = self.clone();
        result
    }

    pub fn update(&mut self, uuid: Uuid, entry_price_i64: i64) -> Result<(), std::io::Error> {
        if self.hash.insert(uuid) {
            self.hash.remove(&uuid);
            Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Key does not exist",
            ))
        } else {
            let key_index = self
                .sorted_order
                .iter()
                .position(|&(x, _y)| x == uuid)
                .unwrap();

            self.sorted_order[key_index].1 = entry_price_i64;
            Ok(())
        }
    }
    // removes the return items
    pub fn search_lt(&mut self, price: i64) -> Vec<Uuid> {
        self.sort();
        let result: Vec<Uuid> = Vec::new();
        if self.min_price < price {
            let key_index = self.sorted_order.iter().rposition(|&(_x, y)| y <= price);
            if key_index.is_some() {
                let result_vec: Vec<(Uuid, i64)> =
                    self.sorted_order.drain(0..key_index.unwrap()).collect();
                let (left, _): (Vec<Uuid>, Vec<i64>) = result_vec.iter().cloned().unzip();
                self.hash.retain(|&x| left.contains(&x) == false);
                return left;
            }
        }
        result
    }
    // removes the return items
    pub fn search_gt(&mut self, price: i64) -> Vec<Uuid> {
        self.sort();
        let result: Vec<Uuid> = Vec::new();
        if self.max_price > price {
            let key_index = self.sorted_order.iter().position(|&(_x, y)| y >= price);
            if key_index.is_some() {
                let result_vec: Vec<(Uuid, i64)> = self
                    .sorted_order
                    .drain(key_index.unwrap()..self.len)
                    .collect();
                let (left, _): (Vec<Uuid>, Vec<i64>) = result_vec.iter().cloned().unzip();
                self.hash.retain(|&x| left.contains(&x) == false);
                return left;
            }
        } else {
        }
        result
    }
}
