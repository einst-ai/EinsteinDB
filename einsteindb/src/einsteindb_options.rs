//Copyright 2020 WHTCORPS INC
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use crate::einsteindb::EinsteinMerkleEngine;
use einsteindb_promises::DBOptions;
use einsteindb_promises::DBOptionsExt;
use einsteindb_promises::Result;
use einsteindb_promises::EinsteinMerkleDBOptions;
use einstein_merkle::DBOptions as RawDBOptions;
use einstein_merkle::EinsteinMerkleDBOptions as RawEinsteinMerkleDBOptions;

impl DBOptionsExt for EinsteinMerkleEngine {
    type DBOptions = einstein_merkleOptions;

    fn get_db_options(&self) -> Self::DBOptions {
        einstein_merkleOptions::from_raw(self.as_inner().get_db_options())
    }
    fn set_db_options(&self, options: &[(&str, &str)]) -> Result<()> {
        self.as_inner()
            .set_db_options(options)
            .map_err(|e| box_err!(e))
    }
}

pub struct einstein_merkleOptions(RawDBOptions);

impl einstein_merkleOptions {
    pub fn from_raw(raw: RawDBOptions) -> einstein_merkleOptions {
        einstein_merkleOptions(raw)
    }

    pub fn into_raw(self) -> RawDBOptions {
        self.0
    }
}

impl DBOptions for einstein_merkleOptions {
    type EinsteinMerkleDBOptions = LmdbEinsteinMerkleDBOptions;

    fn new() -> Self {
        einstein_merkleOptions::from_raw(RawDBOptions::new())
    }

    fn get_max_background_jobs(&self) -> i32 {
        self.0.get_max_background_jobs()
    }

    fn get_rate_bytes_per_sec(&self) -> Option<i64> {
        self.0.get_rate_bytes_per_sec()
    }

    fn set_rate_bytes_per_sec(&mut self, rate_bytes_per_sec: i64) -> Result<()> {
        self.0
            .set_rate_bytes_per_sec(rate_bytes_per_sec)
            .map_err(|e| box_err!(e))
    }

    fn set_EinsteinMerkledb_options(&mut self, opts: &Self::EinsteinMerkleDBOptions) {
        self.0.set_EinsteinMerkledb_options(opts.as_raw())
    }
}

pub struct LmdbEinsteinMerkleDBOptions(RawEinsteinMerkleDBOptions);

impl LmdbEinsteinMerkleDBOptions {
    pub fn from_raw(raw: RawEinsteinMerkleDBOptions) -> LmdbEinsteinMerkleDBOptions {
        LmdbEinsteinMerkleDBOptions(raw)
    }

    pub fn as_raw(&self) -> &RawEinsteinMerkleDBOptions {
        &self.0
    }
}

impl EinsteinMerkleDBOptions for LmdbEinsteinMerkleDBOptions {
    fn new() -> Self {
        LmdbEinsteinMerkleDBOptions::from_raw(RawEinsteinMerkleDBOptions::new())
    }

    fn set_min_blob_size(&mut self, size: u64) {
        self.0.set_min_blob_size(size)
    }
}
