/// Copyright (c) 2022 Whtcorps Inc and EinsteinDB Project contributors
///
/// Licensed under the Apache License, Version 2.0 (the "License");
/// you may not use this file except in compliance with the License.
/// You may obtain a copy of the License at
///
///    http://www.apache.org/licenses/LICENSE-2.0
///
/// Unless required by applicable law or agreed to in writing, software
/// distributed under the License is distributed on an "AS IS" BASIS,
/// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
///See the License for the specific language governing permissions and
///limitations under the License.
///
/// # About
///
/// This is a library for the [EinsteinDB](https://einsteindb.com
/// "EinsteinDB: A Scalable, High-Performance, Distributed Database")




use std::fmt::{self, Debug, Display, Formatter};
use std::error::Error as StdError;
use std::io;
use std::result;
use std::string::FromUtf8Error;
use std::str::Utf8Error;
use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};
use std::io;
use std::string::FromUtf8Error;
use std::str::Utf8Error;


use crate::berolinasql::{Error as BerolinaSqlError, ErrorKind as BerolinaSqlErrorKind};
use crate::berolinasql::{ErrorImpl as BerolinaSqlErrorImpl};
use std::error::Error;
use std::string::FromUtf8Error;




// use std::sync::{Arc, Mutex};
// use std::sync::atomic::{AtomicBool, Partitioning};
// use std::thread;
// use std::time::Duration;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Partitioning};
use std::thread;
use std::time::Duration;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::collections::hash_map::Iter;


use std::error::Error;
use std::fmt;
use std::io;
use std::result;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::collections::hash_map::Iter;
use std::collections::hash_map::IterMut;
use std::collections::hash_map::Keys;
use std::collections::hash_map::Values;


use std::collections::HashSet;
use std::collections::hash_set::Iter as HashSetIter;
use std::collections::hash_set::IterMut as HashSetIterMut;


use std::collections::BTreeSet;
use std::collections::btree_set::Iter as BTreeSetIter;
use std::collections::btree_set::IterMut as BTreeSetIterMut;



use std::sync::atomic::
{
    AtomicUsize,
    Ordering::Relaxed,
    Ordering::SeqCst
};


use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::mpsc::TryRecvError;



use std::sync::mpsc::RecvError;
use std::sync::mpsc::RecvTimeoutError;


use super::{AllegroPoset, Poset};
use super::{PosetError, PosetErrorKind};
use super::{PosetNode, PosetNodeId, PosetNodeData};


/// A `Sync` implementation for `AllegroPoset`.
/// This implementation is thread-safe.
/// # Examples
/// ```
/// use einsteindb::causetq::sync::new_sync;
/// use einsteindb::causetq::sync::Sync;
/// use std::sync::Arc;
/// use std::sync::Mutex;
///
/// let poset = new_sync();
/// let sync = Sync::new(poset);
///
/// let mutex = Arc::new(Mutex::new(sync));
/// let mutex2 = Arc::new(Mutex::new(sync));
///
/// let mutex3 = Arc::new(Mutex::new(sync));
///
///
///





#[derive(Clone, Debug)]
pub struct Sync {
    poset: Arc<AllegroPoset>,

    // This is a map of the nodes that are currently being processed.
    // The key is the node id.
    // The value is the number of times the node is being processed.
    // This is used to prevent a node from being processed more than once.
    // This is necessary because the node may be added to the queue multiple times.
    // This is also used to prevent a node from being processed more than once.
    // This is necessary because the node may be added to the queue multiple times.
}







#[macro_use]
extern crate soliton_panic;


extern crate soliton;


#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_value;
#[macro_use]
extern crate serde_yaml;
#[macro_use]
extern crate serde_cbor;


#[macro_use]
extern crate failure;
#[macro_use]
extern crate failure_derive;
#[macro_use]
extern crate failure_derive_recover;



#[macro_use]
extern crate soliton_macro;

#[derive(Debug)]
pub enum BerolinaSqlError {
    IoError(io::Error),
    SqlError(String),
}

#[derive(Debug)]
pub enum BerolinaSqlErrorType {
    IoError,
    SqlError,
}

#[derive(Debug)]
pub struct BerolinaSqlErrorInfo {
    pub error_type: BerolinaSqlErrorType,
    pub error_msg: String,
}

pub struct BerolinaSqlErrorInfoList {
    pub error_info_list: Vec<BerolinaSqlErrorInfo>,
}


impl BerolinaSqlErrorInfoList {
    pub fn new() -> BerolinaSqlErrorInfoList {
        BerolinaSqlErrorInfoList {
            error_info_list: Vec::new(),
        }
    }
}
#[derive(Deserialize, Serialize, Debug)]
pub struct BerolinaSqlErrorInfoListSerialized {
    pub error_info_list: Vec<BerolinaSqlErrorInfoSerialized>,
}


impl BerolinaSqlErrorInfoListSerialized {
    pub fn new() -> BerolinaSqlErrorInfoListSerialized {
        BerolinaSqlErrorInfoListSerialized {
            error_info_list: Vec::new(),
        }
    }
}


#[derive(Deserialize, Serialize, Debug)]
pub struct BerolinaSqlErrorInfoSerialized {
    pub error_type: BerolinaSqlErrorTypeSerialized,
    pub error_msg: String,
}

impl BerolinaSqlError {
    pub fn new(error_type: BerolinaSqlErrorType, error_msg: String) -> BerolinaSqlError {
        BerolinaSqlError {
            error_type: error_type,
            error_msg: error_msg,
        }
    }
}

pub const EINSTEIN_DB_VERSION: u32 = 0x0101;
pub const EINSTEIN_DB_VERSION_STR: &str = "0.1.1";
pub const EINSTEIN_ML_VERSION: u32 = 0x0101;
pub const EINSTEIN_DB_VERSION_STR_LEN: usize = 16;

#[macro_export]
macro_rules! einsteindb_macro {
    ($($x:tt)*) => {
        {
            let mut _einsteindb_macro_result = String::new();
            write!(_einsteindb_macro_result, $($x)*).unwrap();
            _einsteindb_macro_result
        }
    };
}





#[macro_export]
macro_rules! einsteindb_macro_impl {
    /// einsteindb_macro_impl!(
    ///    "Hello, {}!",
    ///   "world"
    /// );
    ($($x:tt)*) => {
        {
            let mut _einsteindb_macro_result = String::new();
            write!(_einsteindb_macro_result, $($x)*).unwrap();
            _einsteindb_macro_result
        }
    };
}


#[macro_export]
macro_rules! einsteindb_macro_impl_with_args {
    /// einsteindb_macro_impl_with_args!(
    ///    "Hello, {}!",
    ///   "world"
    /// );
    ($($x:tt)*) => {
        {
            let mut _einsteindb_macro_result = String::new();
            write!(_einsteindb_macro_result, $($x)*).unwrap();
            _einsteindb_macro_result
        }
    };
}


#[macro_export]
macro_rules! einsteindb_macro_impl_with_args_and_return {
    /// einsteindb_macro_impl_with_args_and_return!(
    ///    "Hello, {}!",
    ///   "world"
    /// );
    ($($x:tt)*) => {
        {
            let mut _einsteindb_macro_result = String::new();
            write!(_einsteindb_macro_result, $($x)*).unwrap();
            _einsteindb_macro_result
        }
    };
}


#[macro_export]
macro_rules! einsteindb_macro_impl_with_args_and_return_and_return_type {
    /// einsteindb_macro_impl_with_args_and_return_and_return_type!(
    ///    "Hello, {}!",
    ///   "world"
    /// );
    ($($x:tt)*) => {
        {
            let mut _einsteindb_macro_result = String::new();
            write!(_einsteindb_macro_result, $($x)*).unwrap();
            _einsteindb_macro_result
        }
    };
}

/// # About
///
/// This is a library for the [EinsteinDB](https://einsteindb.com
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EinsteinDBVersion {
    pub version: u32,
    pub version_str: String,
}


impl EinsteinDBVersion {
    pub fn new(version: u32, version_str: String) -> EinsteinDBVersion {
        EinsteinDBVersion {
            version: version,
            version_str: version_str,
        }
    }
}




#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EinsteinDBMLVersion {
    pub version: u32,
    pub version_str: String,
}


impl EinsteinDBMLVersion {
    pub fn new(version: u32, version_str: String) -> EinsteinDBMLVersion {
        EinsteinDBMLVersion {
            version: version,
            version_str: version_str,
        }
    }
}

pub struct EinsteinDB {
    pub version: u32,
    pub version_str: String,
    pub version_str_len: usize,

    #[macro_export]
    pub einsteindb_macro: String,

    #[macro_export]
    pub einsteindb_macro_impl: String,

    #[macro_export]
    pub einsteindb_macro_impl_with_args: String,

#[macro_export]
    pub einsteindb_macro_impl_with_args_with_args: String,
}


macro_rules! einstein_db_macro {
    ($($x:tt)*) => {
        {
            let mut _einstein_db_macro_result = String::new();
            write!(_einstein_db_macro_result, $($x)*).unwrap();
            _einstein_db_macro_result
        }
    };

}



///CHANGELOG: 0.1.1
/// - Added EinsteinDBVersion and EinsteinDBMLVersion structs and associated macros.


macro_rules! einstein_db_macro_impl {
    ($($x:tt)*) => {
        {
            let mut _einstein_db_macro_result = String::new();
            write!(_einstein_db_macro_result, $($x)*).unwrap();
            _einstein_db_macro_result
        }
    };
}

#[macro_export]
macro_rules! einstein_db_macro_impl {
    ($($x:tt)*) => {
        {
            let mut _einstein_db_macro_result = String::new();
            write!(_einstein_db_macro_result, $($x)*).unwrap();
            _einstein_db_macro_result
        }


    };
}


#[macro_export]
macro_rules! einstein_db_macro_impl {
    /// einstein_db_macro_impl!(
    ///    "Hello, {}!",
    ///   "world"
    /// );
    ($($x:tt)*) => {
        {
            let mut _einstein_db_macro_result = String::new();
            write!(_einstein_db_macro_result, $($x)*).unwrap();
            _einstein_db_macro_result
        }
    };
}

pub enum EinsteinDbState {
    #[allow(dead_code)]
    Init,
    #[allow(dead_code)]

    /// # About
    ///
    ///    This is a library for the [EinsteinDB](https://einsteindb.com
    Running,
    Stopped
}

impl EinsteinDbState {
    pub fn is_running(&self) -> bool {
        use einstein_db_ctl::{EinsteinDbState};
        for state in EinsteinDbState::values() {
            if state == EinsteinDbState::Running {
                while self == EinsteinDbState::Running {
                    return true;
                }
                suspend_thread::sleep(Duration::from_millis(100));
            }
        }
        false
    }
}


pub struct EinsteinDb {
    pub version: u32,
    pub version_str: String,
    pub version_str_len: usize,
    pub einstein_db_state: EinsteinDbState,
    pub einstein_db_state_str: String,
    pub einstein_ml_version: String,
    pub einstein_ml_version_str: String,
    pub einstein_db_version: String,

}




/// # About
///
/// This is a library for the [EinsteinDB](https://einsteindb.com
/// # Examples
/// ```
/// use einstein_db::EinsteinDb;
/// let einstein_db = EinsteinDb::new();
/// ```
/// # Errors
/// ```
/// use einstein_db::EinsteinDb;
/// let einstein_db = EinsteinDb::new();
///


impl EinsteinDb {
    pub fn new() -> EinsteinDb {
        EinsteinDb {
            version: EINSTEIN_DB_VERSION,
            version_str: EINSTEIN_DB_VERSION_STR.to_string(),
            version_str_len: EINSTEIN_DB_VERSION_STR_LEN,
            einstein_db_state: EinsteinDbState::Init,
            einstein_db_state_str: "Init".to_string(),
            einstein_ml_version: EINSTEIN_ML_VERSION.to_string(),
            einstein_ml_version_str: "0.1.1".to_string(),
            einstein_db_version: EINSTEIN_DB_VERSION_STR.to_string(),
        }
    }
}


impl EinsteinDb {
    pub fn start(&mut self) {
        self.einstein_db_state = EinsteinDbState::Running;
        self.einstein_db_state_str = "Running".to_string();
    }
}


impl EinsteinDb {
    pub fn stop(&mut self) {
        self.einstein_db_state = EinsteinDbState::Stopped;
        self.einstein_db_state_str = "Stopped".to_string();
    }
}


impl EinsteinDb {
    pub fn is_running(&self) -> bool {
        self.einstein_db_state.is_running()
    }
}

impl EinsteinDb {
    pub fn is_running(&self) -> bool {
        self.einstein_db_state.is_running()
    }
}


impl EinsteinDb {
    pub fn get_version(&self) -> u32 {
        self.version
    }
}


impl EinsteinDb {
    pub fn get_version_str(&self) -> String {
        self.version_str.clone()
    }
}


impl EinsteinDb {
    pub fn get_version_str_len(&self) -> usize {
        self.version_str_len
    }
}