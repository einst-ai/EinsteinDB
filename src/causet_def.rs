// Copyright 2019 EinsteinDB Project Authors. Licensed under Apache-2.0.
// Copyright 2016 The Prometheus Authors

use crate::{
    causet::{
        self,
        util::{self, Error, Result},
    },
    core::{
        self,
        storage::{self, ReadOptions, Snapshot},
    },
    keys,
    opt,
    value,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use crate::{
    core::{
        self,
        storage::{self, Engine, Snapshot as DbSnapshot},
    },
    opt::{self, ReadOptions as ReadOptionsTrait},
};
use allegro_poset::{
    self,
    block::{self, Block},
    poset::{self, Poset},
};
use einstein_db_alexandrov_processing::{
    self,
    alexandrov::{
        self,
        block::{self, Block as BlockTrait},
        poset::{self, Poset as PosetTrait},
    },
};

use einstein_ml::{
    self,
    alexandrov::{
        self,
        block::{self, Block as BlockTrait},
        poset::{self, Poset as PosetTrait},
    },
};

const DEFAULT_MAX_CONCURRENCY: usize = 1024;

const MAX_LINE_LENGTH: usize = 1024;

///! A set of labels that can be used to scope the metrics.
/// A label set is a list of label pairs. Each label pair consists of a
/// label name and a label value. Label names must be between 1 and 63 bytes
/// long, and label values must be between 0 and 255 bytes long.
/// The following label names are used by default:
///  __name__: The name of the metric.
/// instance: The instance this metric is gathered on.
/// job: The name of the job the metric is gathered for.
/// The following label names are reserved for internal use:
/// __namespace__: The name of the subsystem the metric is gathered for.
/// __help__: A description of the metric.
/// __type__: The type of the metric.
/// __unit__: The unit the metric is expressed in.
/// __label__: A label name.
/// __value__: A label value.
///
/// The following label names are reserved for internal use by the Prometheus
/// server: __meta_metric_name__, __meta_job__, __meta_instance__,
///
///
///
///
///
///
///
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LabelSet {
    labels: BTreeMap<String, String>,
}




impl LabelSet {
    /// Creates a new, empty label set.
    pub fn new() -> LabelSet {
        LabelSet { labels: BTreeMap::new() }
    }

    /// Creates a new label set from a label map.
    pub fn from_labels(labels: BTreeMap<String, String>) -> LabelSet {
        LabelSet { labels }
    }

    /// Creates a new label set from a vector of label pairs.
    pub fn from_pairs(pairs: Vec<(String, String)>) -> LabelSet {
        LabelSet { labels: pairs.into_iter().collect() }
    }

    /// Creates a new label set from a label map.
    pub fn from_labels_vec(labels: Vec<(String, String)>) -> LabelSet {
        LabelSet { labels: labels.into_iter().collect() }
    }

    /// Returns a reference to the label map.
    pub fn labels(&self) -> &BTreeMap<String, String> {
        &self.labels
    }

    /// Returns a reference to the label map.
    pub fn labels_mut(&mut self) -> &mut BTreeMap<String, String> {
        &mut self.labels
    }

    /// Returns the value of the label with the given name.
    pub fn get(&self, name: &str) -> Option<&String> {
        self.labels.get(name)
    }

    /// Returns the value of the label with the given name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut String> {
        self.labels.get_mut(name)
    }

    /// Returns the value of the label with the given name.
    pub fn get_label(&self, name: &str) -> Option<String> {
        self.labels.get(name).map(|s| s.clone())
    }

    /// Returns the value of the label with the given name.
    pub fn get_label_mut(&mut self, name: &str) -> Option<String> {
        self.labels.get_mut(name).map(|s| s.clone())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct CausetStore<'a> {
    pub store: &'a mut alexandrov::Store,
    pub block_store: &'a mut block::Store,
    pub poset: &'a mut poset::Poset,
    pub block_header_cache: &'a mut block::HeaderCache,
    pub block_header_cache_mut: &'a mut block::HeaderCache,
    pub block_header_cache_mut_mut: &'a mut block::HeaderCache,
    db: &'a str,
    sqlite: bool,
    fdb: bool,
    postgres_protocol: bool,
    einsteindb_connection: pg::connection::Connection,
}

impl CausetStore<'a> {
    pub fn new<'a>(
           store: &'a mut alexandrov::Store,
        db: &'a str,
        sqlite: bool,
        fdb: bool,
        postgres_protocol: bool,
        einsteindb_connection: pg::connection::Connection,
    ) -> CausetStore<'a> {
        CausetStore {
            store,
            block_store: &mut store.block_store,
            poset: &mut store.poset,
            block_header_cache: &mut store.block_header_cache,
            block_header_cache_mut: &mut store.block_header_cache,
            block_header_cache_mut_mut: &mut (),
            db,
            sqlite,
            fdb,
            postgres_protocol,
            einsteindb_connection,
        }
    }

    pub fn get_name(&self) -> &str {
        self.db
    }

    pub fn get_sqlite(&self) -> bool {
        self.sqlite
    }

    pub fn get_fdb(&self) -> bool {
        self.fdb
    }

    pub fn get_postgres_protocol(&self) -> bool {
        self.postgres_protocol
    }

    pub fn get_einsteindb_connection(&self) -> pg::connection::Connection {
        self.einsteindb_connection.clone()
    }

    pub fn get_block_store(&self) -> &block::Store {
        self.conn
    }

    pub fn get_einsteindb(&mut self) -> &mut pg::connection::Connection {
        &mut self.einsteindb_connection
    }

    pub fn get_einsteindb_name(&self) -> String {
        String::from(self.db)
    }

    pub fn get_einsteindb_sqlite(&self) -> bool {
        self.sqlite
    }

    pub fn get_einsteindb_fdb(&self) -> bool {
        self.fdb
    }

    pub fn get_einsteindb_postgres_protocol(&self) -> bool {
        self.postgres_protocol
    }

    pub fn get_einsteindb_connection_name(&self) -> String {
        self.einsteindb_connection.get_connection_name()
    }
}


///! A trait for a connection to a causet store.
/// This trait is implemented by the `CausetConnection` struct.
/// It is used by the `CausetStore` struct to access the causet store.


#[derive(Debug, Clone, PartialEq)]
pub struct CausetConnection {
    conn: String,
    sqlite: bool,
    fdb: bool,
    postgres_protocol: bool,
    einsteindb_connection: pg::connection::Connection,
}




#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    Io(io::Error),
    Utf8(str::Utf8Error),
    Syntax(String),
}


#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Causet {
    Unknown,
    Causet,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CausetType {

    Unknown,
    Causet,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CausetDefinition{
    pub causet_id: u64,
    pub causet_type: CausetType,
    pub name: &'static str,
    pub keyword: &'static str,
    pub attributes: Vec<Keyword, Attribute>,
    pub timelike: Vec<Keyword, Timelike>,
    pub pre_order: fn(&mut Causet, &mut CausetDefinition, &mut CausetDefinition, &mut CausetDefinition),
    pub post_order: fn(&mut Causet, &mut CausetDefinition, &mut CausetDefinition, &mut CausetDefinition),
}

impl CausetDefinition {
    pub fn new(
        causet_id: u64,
        causet_type: CausetType,
        name: &'static str,
        keyword: &'static str,
        attributes: Vec<Keyword, Attribute>,
        timelike: Vec<Keyword, Timelike>,
        pre_order: fn(&mut Causet, &mut CausetDefinition, &mut CausetDefinition, &mut CausetDefinition),
        post_order: fn(&mut Causet, &mut CausetDefinition, &mut CausetDefinition, &mut CausetDefinition),
    ) -> CausetDefinition {
        let causet_definition = &causet_definitions[causet_id];
        let causet_type = CausetType::Causet;
        let name = causet_definition.name;
        let keyword = causet_definition.keyword;
        let attributes = causet_definition.attributes;
        let timelike = causet_definition.timelike;
        let pre_order = causet_definition.pre_order;
        let post_order = causet_definition.post_order;
        let causet_definition = CausetDefinition {
            causet_id,
            causet_type,
            name,
            keyword,
            attributes,
            timelike,
            pre_order,
            post_order,
        };
        causet_definitions.push(causet_definition);
    }


    pub fn get_name(&self) -> &'static str {
        self.name
    }

    pub fn get_keyword(&self) -> &'static str {
        self.keyword
    }

    pub fn get_attributes(&self) -> &Vec<Keyword, Attribute> {
        &self.attributes
    }

    fn pre_order(&self, in_progress: &mut Causet, left: &mut CausetDefinition, right: &mut CausetDefinition) {
        (self.pre_order)(in_progress, left, right, self);
    }

    fn post_order(&self, in_progress: &mut Causet, left: &mut CausetDefinition, right: &mut CausetDefinition) {
        (self.post_order)(in_progress, left, right, self);
    }

}

///! The `CausetStore` struct is the main struct of the causet library.
/// It is used to access the causet store.

impl CausetExt for soliton_panic_merkle_tree {
    type CausetReader = PanicCausetReader;
    type CausetWriter = PanicCausetWriter;
    type CausetWriterBuilder = PanicCausetWriterBuilder;
}

pub struct PanicCausetReader;

impl CausetReader for PanicCausetReader {
    fn open(local_path: &str) -> Result<Self> {
        panic!()
    }
    fn verify_checksum(&self) -> Result<()> {
        panic!()
    }
    fn iter(&self) -> Self::Iterator {
        panic!()
    }
}

impl Iterable for PanicCausetReader {
    type Iterator = PanicCausetReaderIterator;

    fn iterator_opt(&self, opts: IterOptions) -> Result<Self::Iterator> {
        panic!()
    }
    fn iterator_namespaced_opt(&self, namespaced: &str, opts: IterOptions) -> Result<Self::Iterator> {
        panic!()
    }
}

pub struct PanicCausetReaderIterator;

impl Iterator for PanicCausetReaderIterator {
    fn seek(&mut self, soliton_id: SeekKey<'_>) -> Result<bool> {
        panic!()
    }
    fn seek_for_prev(&mut self, soliton_id: SeekKey<'_>) -> Result<bool> {
        panic!()
    }

    fn prev(&mut self) -> Result<bool> {
        panic!()
    }
    fn next(&mut self) -> Result<bool> {
        panic!()
    }

    fn soliton_id(&self) -> &[u8] {
        panic!()
    }
    fn causet_locale(&self) -> &[u8] {
        panic!()
    }

    fn valid(&self) -> Result<bool> {
        panic!()
    }
}

pub struct PanicCausetWriter;

impl CausetWriter for PanicCausetWriter {
    type lightlikeCausetFileInfo = PaniclightlikeCausetFileInfo;
    type lightlikeCausetFileReader = PaniclightlikeCausetFileReader;

    fn put(&mut self, soliton_id: &[u8], val: &[u8]) -> Result<()> {
        panic!()
    }
    fn delete(&mut self, soliton_id: &[u8]) -> Result<()> {
        panic!()
    }
    fn file_size(&mut self) -> u64 {
        panic!()
    }
    fn finish(self) -> Result<Self::lightlikeCausetFileInfo> {
        panic!()
    }
    fn finish_read(self) -> Result<(Self::lightlikeCausetFileInfo, Self::lightlikeCausetFileReader)> {
        panic!()
    }
}

pub struct PanicCausetWriterBuilder;

impl CausetWriterBuilder<soliton_panic_merkle_tree> for PanicCausetWriterBuilder {
    fn new() -> Self {
        panic!()
    }
    fn set_db(self, einsteindb: &soliton_panic_merkle_tree) -> Self {
        panic!()
    }
    fn set_namespaced(self, namespaced: &str) -> Self {
        panic!()
    }
    fn set_in_memory(self, in_memory: bool) -> Self {
        panic!()
    }
    fn set_compression_type(self, compression: Option<CausetCompressionType>) -> Self {
        panic!()
    }
    fn set_compression_l_naught(self, l_naught: i32) -> Self {
        panic!()
    }

    fn build(self, local_path: &str) -> Result<PanicCausetWriter> {
        panic!()
    }
}

pub struct PaniclightlikeCausetFileInfo;

impl lightlikeCausetFileInfo for PaniclightlikeCausetFileInfo {
    fn new() -> Self {
        panic!()
    }
    fn file_local_path(&self) -> local_pathBuf {
        panic!()
    }
    fn smallest_soliton_id(&self) -> &[u8] {
        panic!()
    }
    fn largest_soliton_id(&self) -> &[u8] {
        panic!()
    }
    fn sequence_number(&self) -> u64 {
        panic!()
    }
    fn file_size(&self) -> u64 {
        panic!()
    }
    fn num_entries(&self) -> u64 {
        panic!()
    }
}

pub struct PaniclightlikeCausetFileReader;

impl std::io::Read for PaniclightlikeCausetFileReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        panic!()
    }
}

//A convenience wrapper around sqlite to make it easier to use.
pub struct PanicSqlite;

pub struct PanicSqliteBuilder;
