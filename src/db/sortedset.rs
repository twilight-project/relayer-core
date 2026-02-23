#![allow(dead_code)]
#![allow(unused_imports)]
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use stopwatch::Stopwatch;
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
                std::io::ErrorKind::NotFound,
                "Key already exist",
            ))
        }
    }

    pub fn remove(&mut self, uuid: Uuid) -> Result<(Uuid, i64), std::io::Error> {
        // let mut value: Vec<(Uuid, i64)> = Vec::new();

        if self.hash.remove(&uuid) {
            let key_index = self.sorted_order.iter().position(|&(x, _y)| x == uuid);
            if key_index.is_some() {
                let mut value: Vec<(Uuid, i64)> = self
                    .sorted_order
                    .drain(key_index.unwrap()..(key_index.unwrap() + 1))
                    .collect();
                match value.pop() {
                    Some((id, price)) => {
                        self.len -= 1;
                        Ok((id, price))
                    }
                    None => Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Key does not exist",
                    )),
                }
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Key does not exist",
                ));
            }
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Key does not exist",
            ));
        }
    }

    pub fn sort(&mut self) {
        self.sorted_order.sort_by_key(|p| p.1);
        self.len = self.sorted_order.len();
        if self.len > 0 {
            self.min_price = self.sorted_order[0].1;
            self.max_price = self.sorted_order[self.len - 1].1;
        }
    }
    pub fn sorthash(&mut self) {
        self.sorted_order.sort_by_key(|p| p.0);
        self.len = self.sorted_order.len();
        if self.len > 0 {
            self.min_price = self.sorted_order[0].1;
            self.max_price = self.sorted_order[self.len - 1].1;
        }
    }

    pub fn read(&mut self) -> SortedSet {
        self.sort();
        let result = self.clone();
        result
    }

    pub fn update(&mut self, uuid: Uuid, price: i64) -> Result<(), std::io::Error> {
        if self.hash.contains(&uuid) {
            let key_index = self
                .sorted_order
                .iter()
                .position(|&(x, _y)| x == uuid)
                .unwrap();
            self.sorted_order[key_index].1 = price;
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Key does not exist",
            ))
        }
    }

    pub fn update_bulk(&mut self, mut value: Vec<(Uuid, i64)>) -> Result<(), std::io::Error> {
        self.sorthash();
        value.sort_by_key(|p| p.0);
        let db = self.sorted_order.clone();
        let mut db_iter = db.iter();
        let mut cursor: usize = 0;
        for (uuid, price) in value {
            if self.hash.contains(&uuid) {
                let relative_index = db_iter.position(|&(x, _y)| x == uuid).unwrap();
                let absolute_index = cursor + relative_index;
                self.sorted_order[absolute_index].1 = price;
                cursor = absolute_index + 1;
            }
        }
        Ok(())
    }

    // removes the return items
    pub fn search_lt(&mut self, price: i64) -> Vec<Uuid> {
        self.sort();
        let result: Vec<Uuid> = Vec::new();
        if self.min_price < price {
            let key_index = self.sorted_order.iter().rposition(|&(_x, y)| y <= price);
            if key_index.is_some() {
                let result_vec: Vec<(Uuid, i64)> =
                    self.sorted_order.drain(0..key_index.unwrap() + 1).collect();
                let (left, _): (Vec<Uuid>, Vec<i64>) = result_vec.iter().cloned().unzip();
                for x in left.clone() {
                    self.hash.remove(&x);
                }
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
                for x in left.clone() {
                    self.hash.remove(&x);
                }
                return left;
            }
        } else {
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_bulk_second_entry_writes_wrong_index() {
        let mut set = SortedSet::new();

        // Create 3 deterministic UUIDs that sort in a known order
        let uuid_a = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let uuid_b = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();
        let uuid_c = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();
        let uuid_d = Uuid::parse_str("00000000-0000-0000-0000-000000000004").unwrap();
        let uuid_e = Uuid::parse_str("00000000-0000-0000-0000-000000000005").unwrap();
        let uuid_f = Uuid::parse_str("00000000-0000-0000-0000-000000000006").unwrap();

        set.add(uuid_a, 100).unwrap();
        set.add(uuid_b, 200).unwrap();
        set.add(uuid_c, 300).unwrap();
        set.add(uuid_d, 400).unwrap();
        set.add(uuid_e, 500).unwrap();
        set.add(uuid_f, 600).unwrap();
        // Update B and C with new prices
        let updates = vec![(uuid_b, 999), (uuid_c, 888), (uuid_f, 555)];
        set.update_bulk(updates).unwrap();

        // Sort by UUID to inspect in deterministic order
        set.sorthash();
        println!("set: {:#?}", set.sorted_order);
        // Check results: A should be unchanged, B and C should have new prices
        assert_eq!(
            set.sorted_order[0],
            (uuid_a, 100),
            "A should be unchanged at price 100"
        );
        assert_eq!(
            set.sorted_order[1],
            (uuid_b, 999),
            "B should be updated to 999"
        );
        assert_eq!(
            set.sorted_order[2],
            (uuid_c, 888),
            "C should be updated to 888"
        );
    }
}
