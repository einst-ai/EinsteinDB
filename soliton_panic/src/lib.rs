mod snapshot;
mod mvsr;
mod misc;

/// Copyright 2019 EinsteinDB Project Authors.
/// Licensed under Apache-2.0.

//! An example EinsteinDB timelike_storage einstein_merkle_tree.
//!
//! This project is intended to serve as a skeleton for other einstein_merkle_tree
//! implementations. It lays out the complex system of einstein_merkle_tree modules and traits
//! in a way that is consistent with other EinsteinMerkleTrees. To create a new einstein_merkle_tree
//! simply copy the entire directory structure and replace all "Panic*" names
//! with your einstein_merkle_tree's own name; then fill in the implementations; remove
//! the allow(unused) attribute;
//! 




//! The `einstein_merkle_tree` module provides the core API for einstein_merkle_tree.
//! It defines the `EinsteinMerkleTree` trait, which is the core interface for
//! einstein_merkle_tree. It also defines the `EinsteinMerkleTreeReader` trait, which is the
//! core interface for einstein_merkle_tree readers.
//! It also defines the `EinsteinMerkleTreeWriter` trait, which is the core interface for
//! einstein_merkle_tree writers.




use berolina_sql::{
    parser::Parser,
    value::{Value, ValueType},
    error::{Error, Result},
    parser::ParserError,
    value::{ValueRef, ValueRefMut},
    fdb_traits::FdbTrait,
    fdb_traits::FdbTraitImpl,
    pretty,
    io,
    convert::{TryFrom, TryInto},
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};


use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use fdb_traits::{FdbTransactional, FdbTransactionalExt};
use allegro_poset::*;
use std::time::Instant;
use std::thread;
use std::thread::JoinHandle;
use std::thread::Thread;
use std::thread::ThreadId;
use std::thread::ThreadIdRange;
use std::thread::ThreadIdRangeInner;


use haraka256::*;
use soliton_panic::*;


use einstein_db::config::Config;
use EinsteinDB::*;
use super::*;




use std::local_path::local_path;


use super::*;


use einstein_db_alexandrov_processing::{
    index::{
        Index,
        IndexIterator,
        IndexIteratorOptions,
        IndexIteratorOptionsBuilder,
    }
};


use berolina_sql::{
    parser::Parser,
    value::{Value, ValueType},
    error::{Error, Result},
    parser::ParserError,
    value::{ValueRef, ValueRefMut},
    fdb_traits::FdbTrait,
    fdb_traits::FdbTraitImpl,
    pretty,
    io,
    convert::{TryFrom, TryInto},
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};


use itertools::Itertools;


#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub thread_id: ThreadId,
    pub thread: Thread,
    pub join_handle: JoinHandle<()>,
    pub thread_id_range: ThreadIdRange,
    pub thread_id_range_inner: ThreadIdRangeInner,
    pub thread_id_range_inner_inner: ThreadIdRangeInnerInner,
    pub thread_name: String,
    pub thread_name_path: String,
    pub thread_name_name: String,
    pub thread_name_file: String,
    pub thread_name_file_path: String,
    pub thread_name_file_name: String,
    pub thread_name_file_content: String,
}

#[derive(Debug, Clone)]
pub struct ThreadInfoList {
    pub thread_info_list: Vec<ThreadInfo>,
}


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
    pub nonce: u64,
    pub gas_price: u64,
    pub gas_limit: u64,
    pub action: [u8; 32],
    pub data: [u8; 32],
    pub signature: [u8; 32],
    pub hash: [u8; 32],
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicReceipt {
    pub state_root: [u8; 32],
    pub gas_used: u64,
    pub logs: [u8; 32],
    pub bloom: [u8; 32],
    pub error: [u8; 32],
    pub output: [u8; 32],
    pub hash: [u8; 32],
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicLog {
    pub address: [u8; 32],
    pub topics: [u8; 32],
    pub data: [u8; 32],
    pub hash: [u8; 32],
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicLogBloom {
    pub bloom: [u8; 32],
    pub hash: [u8; 32],
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicLogEntry {
    pub address: [u8; 32],
    pub topics: [u8; 32],
    pub data: [u8; 32],
    pub hash: [u8; 32],
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicLogEntryBloom {
    pub bloom: [u8; 32],
    pub hash: [u8; 32],
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicLogTopic {
    pub topic: [u8; 32],
    pub hash: [u8; 32],
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicLogTopicBloom {
    pub bloom: [u8; 32],
    pub hash: [u8; 32],
}




#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicAccountBalance {
    pub balance: u64,
    pub hash: [u8; 32],
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicAccountNonce {
    pub nonce: u64,
    pub hash: [u8; 32],
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicAccountCode {
    pub code: [u8; 32],
    pub hash: [u8; 32],
}



impl PanicTransaction {
    pub fn new(nonce: u64, gas_price: u64, gas_limit: u64, action: [u8; 32], data: [u8; 32], signature: [u8; 32], hash: [u8; 32]) -> Self {
        Self {
            nonce,
            gas_price,
            gas_limit,
            action,
            data,
            signature,
            hash,
        }
    }




        pub fn new_from_bytes(bytes: &[u8]) -> Result<Self, E> {
        let mut parser = Parser::new(bytes);
        let nonce = parser.parse_u64()?;
        let gas_price = parser.parse_u64()?;
        let gas_limit = parser.parse_u64()?;
        let action = parser.parse_bytes(32)?;
        let data = parser.parse_bytes(32)?;
        let signature = parser.parse_bytes(32)?;
        let hash = parser.parse_bytes(32)?;
        Ok(Self {
            nonce,
            gas_price,
            gas_limit,
            action,
            data,
            signature,
            hash,
        })
    }

pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.nonce.to_le_bytes());
        bytes.extend_from_slice(&self.gas_price.to_le_bytes());
        bytes.extend_from_slice(&self.gas_limit.to_le_bytes());
        bytes.extend_from_slice(&self.action);
        bytes.extend_from_slice(&self.data);
        bytes.extend_from_slice(&self.signature);
        bytes.extend_from_slice(&self.hash);
        bytes
    }

}


impl PanicReceipt {

    pub fn new(state_root: [u8; 32], gas_used: u64, logs: [u8; 32], bloom: [u8; 32], error: [u8; 32], output: [u8; 32], hash: [u8; 32]) -> Self {
        Self {
            state_root,
            gas_used,
            logs,
            bloom,
            error,
            output,
            hash,
        }
    }

    pub fn new_from_bytes(bytes: &[u8]) -> Result<Self, E> {
        let mut parser = Parser::new(bytes);
        let state_root = parser.parse_bytes(32)?;
        let gas_used = parser.parse_u64()?;
        let logs = parser.parse_bytes(32)?;
        let bloom = parser.parse_bytes(32)?;
        let error = parser.parse_bytes(32)?;
        let output = parser.parse_bytes(32)?;
        let hash = parser.parse_bytes(32)?;
        Ok(Self {
            state_root,
            gas_used,
            logs,
            bloom,
            error,
            output,
            hash,
        })
    }


    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.state_root);
        bytes.extend_from_slice(&self.gas_used.to_le_bytes());
        bytes.extend_from_slice(&self.logs);
        bytes.extend_from_slice(&self.bloom);
        bytes.extend_from_slice(&self.error);
        bytes.extend_from_slice(&self.output);
        bytes.extend_from_slice(&self.hash);
        bytes
    }


}


impl PanicLog {

    pub fn new(address: [u8; 32], topics: [u8; 32], data: [u8; 32], hash: [u8; 32]) -> Self {
        Self {
            address,
            topics,
            data,
            hash,
        }
    }


    pub fn new_from_bytes(bytes: &[u8]) -> Result<Self, E> {
        let mut parser = Parser::new(bytes);
        let address = parser.parse_bytes(32)?;
        let topics = parser.parse_bytes(32)?;
        let data = parser.parse_bytes(32)?;
        let hash = parser.parse_bytes(32)?;
        Ok(Self {
            address,
            topics,
            data,
            hash,
        })
    }


    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.address);
        bytes.extend_from_slice(&self.topics);
        bytes.extend_from_slice(&self.data);
        bytes.extend_from_slice(&self.hash);
        bytes
    }


}
    pub fn from_raw(sender: Type, receiver: String, value: u64, timestamp: u64) -> Self {
        Self {
            sender,
            receiver,
            value,
            timestamp,
        }
    }




    pub fn from_bytes(bytes: &[u8]) -> Result<Self, E> {
        let mut parser = Parser::new(bytes);
        let sender = parser.parse_u64()?;
        let receiver = parser.parse_string()?;
        let value = parser.parse_u64()?;
        let timestamp = parser.parse_u64()?;
        Ok(Self {
            sender,
            receiver,
            value,
            timestamp,
        })
    }





    pub fn from_raw_data(receiver: String) -> Self {
        PanicTransaction {
            nonce: 0,
            gas_price: 0,
            gas_limit: 0,

            action: [0; 32],
            data: [0; 32],
            signature: [0; 32],
            hash: [0; 32],

        }
    }

    pub(crate) fn from_raw_data_with_nonce(nonce: u64, receiver: String) -> Self {
        PanicTransaction {
            nonce,
            gas_price: 0,
            gas_limit: 0,

            action: [0; 32],
            data: [0; 32],
            signature: [0; 32],
            hash: [0; 32],

        }
    }


    pub fn from_raw_data_with_timestamp_and_receiver(sender: Type, receiver: String, value: u64, timestamp: u64) -> Self {
        PanicTransaction {
            nonce: 0,
            gas_price: 0,
            gas_limit: 0,

            action: [0; 32],
            data: [0; 32],
            signature: [0; 32],
            hash: [0; 32],

        }
    }

    pub fn from_raw_data_with_timestamp_and_receiver_and_value(sender: Type, receiver: String, value: u64, timestamp: u64) -> Self {
        PanicTransaction {
            nonce: 0,
            gas_price: 0,
            gas_limit: 0,

            action: [0; 32],
            data: [0; 32],
            signature: [0; 32],
            hash: [0; 32],


        }
    }




    pub fn from_raw_data_with_timestamp_and_receiver_and_value_and_gas_limit(sender: Type, receiver: String, value: u64, timestamp: u64, gas_limit: u64) -> Self {
        PanicTransaction {
            nonce: 0,
            gas_price: 0,
            gas_limit,

            action: [0; 32],
            data: [0; 32],
            signature: [0; 32],
            hash: [0; 32],
        }
    }



    pub fn into_raw(transaction: PanicTransaction) -> PanicTransaction {
        transaction
    }

    pub fn into_raw_data(transaction: PanicTransaction) -> PanicTransaction {
        transaction
    }


    pub fn into_raw_data_with_timestamp(transaction: PanicTransaction) -> PanicTransaction {
        transaction
    }

    pub fn into_raw_data_with_timestamp_and_value(transaction: PanicTransaction) -> PanicTransaction {
        transaction
    }

    pub fn into_raw_data_with_timestamp_and_receiver(transaction: PanicTransaction) -> PanicTransaction {
        transaction
    }





    pub fn new(sender: Type, receiver: String, value: u64, timestamp: u64) -> Self {
        PanicTransaction {
            nonce: 0,
            gas_price: 0,
            gas_limit: 0,

            action: [0; 32],
            data: [0; 32],
            signature: [0; 32],
            hash: [0; 32],
        }
    }

    pub fn new_data(sender: Type, receiver: String, value: u64) -> Self {
        PanicTransaction {
            nonce: 0,
            gas_price: 0,
            gas_limit: 0,

           action: [0; 32],
              data: [0; 32],
                signature: [0; 32],
                hash: [0; 32],
        }
    }

    pub fn new_data_with_timestamp(sender: Type, receiver: String, value: u64, timestamp: u64) -> Self {
        PanicTransaction {
            nonce: 0,
            gas_price: 0,
            gas_limit: 0,

            action: [0; 32],
            data: [0; 32],
            signature: [0; 32],
            hash: [0; 32],

        }
    }


impl PanicReceipt {
    
    pub fn new(state_root: [u8; 32], gas_used: u64, logs: [u8; 32], bloom: [u8; 32], error: [u8; 32], output: [u8; 32], hash: [u8; 32]) -> Self {
        PanicReceipt {
            state_root,
            gas_used,
            logs,
            bloom,
            error,
            output,
            hash,
        }
    }

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


impl PanicBlockHeaderDB {
    pub fn new(number: u64, parent_hash: String, timestamp: u64) -> PanicBlockHeaderDB {
        PanicBlockHeaderDB {
            number,
            parent_hash,
            timestamp,

        }
    }
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanicBlockHeader {
    pub number: u64,
    pub parent_hash: String,
    pub timestamp: u64,
    pub state_root: [u8; 32],
    pub transactions_root: [u8; 32],
    pub receipts_root: [u8; 32],
    pub logs_bloom: [u8; 32],
    pub difficulty: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub extra_data: String,
    pub mix_hash: String,
    pub nonce: String,
    pub seal_fields: Vec<String>,
    pub sha3_uncles: String,
    pub size: u64,
    pub total_difficulty: u64,
    pub transactions: Vec<String>,
    pub uncles: Vec<String>,
}

///CHANGELOG: Added uncles field_type: Vec<String>
/// CHANGELOG: Added size field_type: u64x2
/// CHANGELOG: Added total_difficulty field_type: u64x2
/// CHANGELOG: Added seal_fields field_type: Vec<String>




