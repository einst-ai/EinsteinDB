// Copyright 2019 EinsteinDB Project Authors. Licensed under Apache-2.0.

//! An example EinsteinDB timelike_storage einstein_merkle_tree.
//!
//! This project is intended to serve as a skeleton for other einstein_merkle_tree
//! implementations. It lays out the complex system of einstein_merkle_tree modules and traits
//! in a way that is consistent with other EinsteinMerkleTrees. To create a new einstein_merkle_tree
//! simply copy the entire directory structure and replace all "Panic*" names
//! with your einstein_merkle_tree's own name; then fill in the implementations; remove
//! the allow(unused) attribute;
#![allow(unused)]
#![cfg_attr(not(feature = "std"), no_std)]


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicAccount {

    pub balance: u64,
    pub nonce: u64,
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicBlock {
    pub number: u64,
    pub parent_hash: [u8; 32],
    pub tx_hash: [u8; 32],
    pub state_hash: [u8; 32],
    pub receipts_hash: [u8; 32],
    pub extra_data: [u8; 32],
    pub logs_block_hash: [u8; 32],
    pub proposer: [u8; 32],
    pub seal: [u8; 32],
    pub hash: [u8; 32],
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicBlockHeader {
    pub number: u64,
    pub parent_hash: [u8; 32],
    pub tx_hash: [u8; 32],
    pub state_hash: [u8; 32],
    pub receipts_hash: [u8; 32],
    pub extra_data: [u8; 32],
    pub logs_block_hash: [u8; 32],
    pub proposer: [u8; 32],
    pub seal: [u8; 32],
    pub hash: [u8; 32],
}




#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicHeader {
    pub number: u64,
    pub parent_hash: [u8; 32],
    pub tx_hash: [u8; 32],
    pub state_hash: [u8; 32],
    pub receipts_hash: [u8; 32],

    pub extra_data: [u8; 32],
    pub logs_block_hash: [u8; 32],


    pub proposer: [u8; 32],
    pub seal: [u8; 32],
    pub hash: [u8; 32],
}








#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicTransaction {

    pub sender: Type,
    pub(crate) receiver: String,
    pub value: u64,
    pub timestamp: u64,
}

impl PanicTransaction {
    
    pub fn new(sender: Type, receiver: String, value: u64, timestamp: u64) -> Self {
        PanicTransaction {
            sender,
            receiver,
            value,
            timestamp,
        }
    }

    pub fn sender(&self) -> &Type {
        &self.sender
    }

    pub fn receiver(&self) -> &String {
        &self.receiver
    }

    pub fn value(&self) -> u64 {
        self.value
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn into_raw(self) -> (Type, String, u64, u64) {
        (self.sender, self.receiver, self.value, self.timestamp)
    }

    pub fn from_raw(sender: Type, receiver: String, value: u64, timestamp: u64) -> Self {
        PanicTransaction {
            sender,
            receiver,
            value,
            timestamp,
        }
    }

    pub fn into_raw_data(self) -> (Type, String, u64) {
        (self.sender, self.receiver, self.value)
    }

    pub fn from_raw_data(sender: Type, receiver: String, value: u64) -> Self {
        PanicTransaction {
            sender,
            receiver,
            value,
            timestamp: 0,
        }
    }

    pub fn into_raw_data_with_timestamp(self) -> (Type, String, u64, u64) {
        (self.sender, self.receiver, self.value, self.timestamp)
    }

    pub fn from_raw_data_with_timestamp(sender: Type, receiver: String, value: u64, timestamp: u64) -> Self {
        PanicTransaction {
            sender,
            receiver,
            value,
            timestamp,
        }
    }

    pub fn into_raw_data_with_timestamp_and_receiver(self) -> (Type, String, u64, u64, String) {
        (self.sender, self.receiver, self.value, self.timestamp, self.receiver)
    }

    pub fn from_raw_data_with_timestamp_and_receiver(sender: Type, receiver: String, value: u64, timestamp: u64, receiver: String) -> Self {
        PanicTransaction {
            sender,
            receiver,
            value,
            timestamp,
        }
    }

    pub fn into_raw_data_with_timestamp_and_receiver_and_value(self) -> (Type, String, u64, u64, String, u64) {
        (self.sender, self.receiver, self.value, self.timestamp, self.receiver, self.value)
    }

    pub fn from_raw_data_with_timestamp_and_receiver_and_value(sender: Type, receiver: String, value: u64, timestamp: u64, receiver: String, value: u64) -> Self {
        PanicTransaction {
            sender,
            receiver,
            value,
            timestamp,
        }
    }

}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicBlockHeader {
    pub number: u64,
    pub parent_hash: String,
    pub timestamp: u64,
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicBlockBody {
    pub transactions: Vec<String>,
}

impl PanicBlockBody {
    pub fn new(transactions: Vec<String>) -> PanicBlockBody {
        PanicBlockBody {
            transactions
        }
    }
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicBlockHeaderDB {
    pub number: u64,
    pub parent_hash: String,
    pub timestamp: u64,
}

