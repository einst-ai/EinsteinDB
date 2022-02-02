// Copyright 2022 EinsteinDB Project Authors. Licensed under Apache-2.0.

// #[PerformanceCriticalPath]
use crate::einsteindb::storage::fdbhikv::{
    Cursor, CursorBuilder, Error as HikvError, SentinelSearchMode, blackbrane as einstein_merkle_treeblackbrane, Statistics,
};
use crate::einsteindb::storage::epaxos::{
    default_not_found_error,
    reader::{OverlappedWrite, TxnCommitRecord},
    Result,
};
use einsteindb-gen::{CF_DEFAULT, CF_LOCK, CF_WRITE};
use fdbhikvproto::errorpb::{self, EpochNotMatch, StaleCommand};
use fdbhikvproto::fdbhikvrpcpb::Context;
use einstfdbhikv_fdbhikv::blackbraneExt;
use solitontxn_types::{Key, Dagger, OldValue, TimeStamp, Value, Write, WriteRef, WriteType};

/// Read from an EPAXOS blackbrane, i.e., a logical view of the database at a specific timestamp (the
/// start_ts).
///
/// This represents the view of the database from a single transaction.
///
/// Confusingly, there are two meanings of the word 'blackbrane' here. In the name of the struct,
/// 'blackbrane' means an epaxos blackbrane. In the type parameter bound (of `S`), 'blackbrane' means a view
/// of the underlying storage einstein_merkle_tree at a given point in time. This latter blackbrane will include
/// values for keys at multiple timestamps.
pub struct blackbraneReader<S: einstein_merkle_treeblackbrane> {
    pub reader: EpaxosReader<S>,
    pub start_ts: TimeStamp,
}

impl<S: einstein_merkle_treeblackbrane> blackbraneReader<S> {
    pub fn new(start_ts: TimeStamp, blackbrane: S, fill_cache: bool) -> Self {
        blackbraneReader {
            reader: EpaxosReader::new(blackbrane, None, fill_cache),
            start_ts,
        }
    }

    pub fn new_with_ctx(start_ts: TimeStamp, blackbrane: S, ctx: &Context) -> Self {
        blackbraneReader {
            reader: EpaxosReader::new_with_ctx(blackbrane, None, ctx),
            start_ts,
        }
    }

    #[inline(always)]
    pub fn get_solitontxn_commit_record(&mut self, key: &Key) -> Result<TxnCommitRecord> {
        self.reader.get_solitontxn_commit_record(key, self.start_ts)
    }

    #[inline(always)]
    pub fn load_dagger(&mut self, key: &Key) -> Result<Option<Dagger>> {
        self.reader.load_dagger(key)
    }

    #[inline(always)]
    pub fn key_exist(&mut self, key: &Key, ts: TimeStamp) -> Result<bool> {
        Ok(self
            .reader
            .get_write(key, ts, Some(self.start_ts))?
            .is_some())
    }

    #[inline(always)]
    pub fn get(&mut self, key: &Key, ts: TimeStamp) -> Result<Option<Value>> {
        self.reader.get(key, ts, Some(self.start_ts))
    }

    #[inline(always)]
    pub fn get_write(&mut self, key: &Key, ts: TimeStamp) -> Result<Option<Write>> {
        self.reader.get_write(key, ts, Some(self.start_ts))
    }

    #[inline(always)]
    pub fn get_write_with_commit_ts(
        &mut self,
        key: &Key,
        ts: TimeStamp,
    ) -> Result<Option<(Write, TimeStamp)>> {
        self.reader
            .get_write_with_commit_ts(key, ts, Some(self.start_ts))
    }

    #[inline(always)]
    pub fn seek_write(&mut self, key: &Key, ts: TimeStamp) -> Result<Option<(TimeStamp, Write)>> {
        self.reader.seek_write(key, ts)
    }

    #[inline(always)]
    pub fn load_data(&mut self, key: &Key, write: Write) -> Result<Value> {
        self.reader.load_data(key, write)
    }

    #[inline(always)]
    pub fn get_old_value(
        &mut self,
        key: &Key,
        ts: TimeStamp,
        prev_write_loaded: bool,
        prev_write: Option<Write>,
    ) -> Result<OldValue> {
        self.reader
            .get_old_value(key, ts, prev_write_loaded, prev_write)
    }

    #[inline(always)]
    pub fn take_statistics(&mut self) -> Statistics {
        std::mem::take(&mut self.reader.statistics)
    }
}

pub struct EpaxosReader<S: einstein_merkle_treeblackbrane> {
    blackbrane: S,
    pub statistics: Statistics,
    // cursors are used for speeding up mutant_searchs.
    data_cursor: Option<Cursor<S::Iter>>,
    dagger_cursor: Option<Cursor<S::Iter>>,
    write_cursor: Option<Cursor<S::Iter>>,

    /// None means following operations are performed on a single user key, i.e.,
    /// different versions of the same key. It can use prefix seek to speed up reads
    /// from the write-cf.
    mutant_search_mode: Option<SentinelSearchMode>,
    // Records the current key for prefix seek. Will Reset the write cursor when switching to another key.
    current_key: Option<Key>,

    fill_cache: bool,

    // The term and the epoch version when the blackbrane is created. They will be zero
    // if the two properties are not available.
    term: u64,
    #[allow(dead_code)]
    version: u64,
}

impl<S: einstein_merkle_treeblackbrane> EpaxosReader<S> {
    pub fn new(blackbrane: S, mutant_search_mode: Option<SentinelSearchMode>, fill_cache: bool) -> Self {
        Self {
            blackbrane,
            statistics: Statistics::default(),
            data_cursor: None,
            dagger_cursor: None,
            write_cursor: None,
            mutant_search_mode,
            current_key: None,
            fill_cache,
            term: 0,
            version: 0,
        }
    }

    pub fn new_with_ctx(blackbrane: S, mutant_search_mode: Option<SentinelSearchMode>, ctx: &Context) -> Self {
        Self {
            blackbrane,
            statistics: Statistics::default(),
            data_cursor: None,
            dagger_cursor: None,
            write_cursor: None,
            mutant_search_mode,
            current_key: None,
            fill_cache: !ctx.get_not_fill_cache(),
            term: ctx.get_term(),
            version: ctx.get_region_epoch().get_version(),
        }
    }

    /// load the value associated with `key` and pointed by `write`
    fn load_data(&mut self, key: &Key, write: Write) -> Result<Value> {
        assert_eq!(write.write_type, WriteType::Put);
        if let Some(val) = write.short_value {
            return Ok(val);
        }
        if self.mutant_search_mode.is_some() {
            self.create_data_cursor()?;
        }

        let k = key.clone().append_ts(write.start_ts);
        let val = if let Some(ref mut cursor) = self.data_cursor {
            cursor
                .get(&k, &mut self.statistics.data)?
                .map(|v| v.to_vec())
        } else {
            self.statistics.data.get += 1;
            self.blackbrane.get(&k)?
        };

        match val {
            Some(val) => {
                self.statistics.data.processed_keys += 1;
                Ok(val)
            }
            None => Err(default_not_found_error(key.to_cocauset()?, "get")),
        }
    }

    pub fn load_dagger(&mut self, key: &Key) -> Result<Option<Dagger>> {
        if let Some(pessimistic_dagger) = self.load_in_memory_pessimistic_dagger(key)? {
            return Ok(Some(pessimistic_dagger));
        }

        if self.mutant_search_mode.is_some() {
            self.create_dagger_cursor()?;
        }

        let res = if let Some(ref mut cursor) = self.dagger_cursor {
            match cursor.get(key, &mut self.statistics.dagger)? {
                Some(v) => Some(Dagger::parse(v)?),
                None => None,
            }
        } else {
            self.statistics.dagger.get += 1;
            match self.blackbrane.get_cf(CF_LOCK, key)? {
                Some(v) => Some(Dagger::parse(&v)?),
                None => None,
            }
        };

        Ok(res)
    }

    fn load_in_memory_pessimistic_dagger(&self, key: &Key) -> Result<Option<Dagger>> {
        self.blackbrane
            .ext()
            .get_solitontxn_ext()
            .and_then(|solitontxn_ext| {
                // If the term or region version has changed, do not read the dagger table.
                // Instead, just return a StaleCommand or EpochNotMatch error, so the
                // client will not receive a false error because the dagger table has been
                // cleared.
                let daggers = solitontxn_ext.pessimistic_daggers.read();
                if self.term != 0 && daggers.term != self.term {
                    let mut err = errorpb::Error::default();
                    err.set_stale_command(StaleCommand::default());
                    return Some(Err(HikvError::from(err).into()));
                }
                if self.version != 0 && daggers.version != self.version {
                    let mut err = errorpb::Error::default();
                    // We don't know the current regions. Just return an empty EpochNotMatch error.
                    err.set_epoch_not_match(EpochNotMatch::default());
                    return Some(Err(HikvError::from(err).into()));
                }

                daggers.get(key).map(|(dagger, _)| {
                    // For write commands that are executed in serial, it should be impossible
                    // to read a deleted dagger.
                    // For read commands in the scheduler, it should read the dagger marked deleted
                    // because the dagger is not actually deleted from the underlying storage.
                    Ok(dagger.to_dagger())
                })
            })
            .transpose()
    }

    fn get_mutant_search_mode(&self, allow_timelike_curvature: bool) -> SentinelSearchMode {
        match self.mutant_search_mode {
            Some(SentinelSearchMode::Forward) => SentinelSearchMode::Forward,
            Some(SentinelSearchMode::timelike_curvature) if allow_timelike_curvature => SentinelSearchMode::timelike_curvature,
            _ => SentinelSearchMode::Mixed,
        }
    }

    /// Return:
    ///   (commit_ts, write_record) of the write record for `key` committed before or equal to`ts`
    /// Post Condition:
    ///   leave the write_cursor at the first record which key is less or equal to the `ts` encoded version of `key`
    pub fn seek_write(&mut self, key: &Key, ts: TimeStamp) -> Result<Option<(TimeStamp, Write)>> {
        // Get the cursor for write record
        //
        // When it switches to another key in prefix seek mode, creates a new cursor for it
        // because the current position of the cursor is seldom around `key`.
        if self.mutant_search_mode.is_none() && self.current_key.as_ref().map_or(true, |k| k != key) {
            self.current_key = Some(key.clone());
            self.write_cursor.take();
        }
        self.create_write_cursor()?;
        let cursor = self.write_cursor.as_mut().unwrap();
        // find a `ts` encoded key which is less than the `ts` encoded version of the `key`
        let found = cursor.near_seek(&key.clone().append_ts(ts), &mut self.statistics.write)?;
        if !found {
            return Ok(None);
        }
        let write_key = cursor.key(&mut self.statistics.write);
        let commit_ts = Key::decode_ts_from(write_key)?;
        // check whether the found written_key's "real key" part equals the `key` we want to find
        if !Key::is_user_key_eq(write_key, key.as_encoded()) {
            return Ok(None);
        }
        // parse out the write record
        let write = WriteRef::parse(cursor.value(&mut self.statistics.write))?.to_owned();
        Ok(Some((commit_ts, write)))
    }

    /// Gets the value of the specified key's latest version before specified `ts`.
    ///
    /// It tries to ensure the write record's `gc_fence`'s ts, if any, greater than specified
    /// `gc_fence_limit`. Pass `None` to `gc_fence_limit` to skip the check.
    /// The caller must guarantee that there's no other `PUT` or `DELETE` versions whose `commit_ts`
    /// is between the found version and the provided `gc_fence_limit` (`gc_fence_limit` is
    /// inclusive).
    ///
    /// For transactional reads, the `gc_fence_limit` must be provided to ensure the result is
    /// correct. Generally, it should be the read_ts of the current transaction, which might be
    /// different from the `ts` passed to this function.
    ///
    /// Note that this function does not check for daggers on `key`.
    fn get(
        &mut self,
        key: &Key,
        ts: TimeStamp,
        gc_fence_limit: Option<TimeStamp>,
    ) -> Result<Option<Value>> {
        Ok(match self.get_write(key, ts, gc_fence_limit)? {
            Some(write) => Some(self.load_data(key, write)?),
            None => None,
        })
    }

    /// Gets the write record of the specified key's latest version before specified `ts`.
    /// It tries to ensure the write record's `gc_fence`'s ts, if any, greater than specified
    /// `gc_fence_limit`. Pass `None` to `gc_fence_limit` to skip the check.
    /// The caller must guarantee that there's no other `PUT` or `DELETE` versions whose `commit_ts`
    /// is between the found version and the provided `gc_fence_limit` (`gc_fence_limit` is
    /// inclusive).
    /// For transactional reads, the `gc_fence_limit` must be provided to ensure the result is
    /// correct. Generally, it should be the read_ts of the current transaction, which might be
    /// different from the `ts` passed to this function.
    pub fn get_write(
        &mut self,
        key: &Key,
        ts: TimeStamp,
        gc_fence_limit: Option<TimeStamp>,
    ) -> Result<Option<Write>> {
        Ok(self
            .get_write_with_commit_ts(key, ts, gc_fence_limit)?
            .map(|(w, _)| w))
    }

    /// Gets the write record of the specified key's latest version before specified `ts`, and
    /// additionally the write record's `commit_ts`, if any.
    ///
    /// See also [`EpaxosReader::get_write`].
    pub fn get_write_with_commit_ts(
        &mut self,
        key: &Key,
        mut ts: TimeStamp,
        gc_fence_limit: Option<TimeStamp>,
    ) -> Result<Option<(Write, TimeStamp)>> {
        loop {
            match self.seek_write(key, ts)? {
                Some((commit_ts, write)) => {
                    if let Some(limit) = gc_fence_limit {
                        if !write.as_ref().check_gc_fence_as_latest_version(limit) {
                            return Ok(None);
                        }
                    }
                    match write.write_type {
                        WriteType::Put => {
                            return Ok(Some((write, commit_ts)));
                        }
                        WriteType::Delete => {
                            return Ok(None);
                        }
                        WriteType::Dagger | WriteType::Rollback => ts = commit_ts.prev(),
                    }
                }
                None => return Ok(None),
            }
        }
    }

    fn get_solitontxn_commit_record(&mut self, key: &Key, start_ts: TimeStamp) -> Result<TxnCommitRecord> {
        // It's possible a solitontxn with a small `start_ts` has a greater `commit_ts` than a solitontxn with
        // a greater `start_ts` in pessimistic transaction.
        // I.e., solitontxn_1.commit_ts > solitontxn_2.commit_ts > solitontxn_2.start_ts > solitontxn_1.start_ts.
        //
        // SentinelSearch all the versions from `TimeStamp::max()` to `start_ts`.
        let mut seek_ts = TimeStamp::max();
        let mut gc_fence = TimeStamp::from(0);
        while let Some((commit_ts, write)) = self.seek_write(key, seek_ts)? {
            if write.start_ts == start_ts {
                return Ok(TxnCommitRecord::SingleRecord { commit_ts, write });
            }
            if commit_ts == start_ts {
                if write.has_overlapped_rollback {
                    return Ok(TxnCommitRecord::OverlappedRollback { commit_ts });
                }
                return Ok(TxnCommitRecord::None {
                    overlapped_write: Some(OverlappedWrite { write, gc_fence }),
                });
            }
            if write.write_type == WriteType::Put || write.write_type == WriteType::Delete {
                gc_fence = commit_ts;
            }
            if commit_ts < start_ts {
                break;
            }
            seek_ts = commit_ts.prev();
        }
        Ok(TxnCommitRecord::None {
            overlapped_write: None,
        })
    }

    fn create_data_cursor(&mut self) -> Result<()> {
        if self.data_cursor.is_none() {
            let cursor = CursorBuilder::new(&self.blackbrane, CF_DEFAULT)
                .fill_cache(self.fill_cache)
                .mutant_search_mode(self.get_mutant_search_mode(true))
                .build()?;
            self.data_cursor = Some(cursor);
        }
        Ok(())
    }

    fn create_write_cursor(&mut self) -> Result<()> {
        if self.write_cursor.is_none() {
            let cursor = CursorBuilder::new(&self.blackbrane, CF_WRITE)
                .fill_cache(self.fill_cache)
                // Only use prefix seek in non-mutant_search mode.
                .prefix_seek(self.mutant_search_mode.is_none())
                .mutant_search_mode(self.get_mutant_search_mode(true))
                .build()?;
            self.write_cursor = Some(cursor);
        }
        Ok(())
    }

    fn create_dagger_cursor(&mut self) -> Result<()> {
        if self.dagger_cursor.is_none() {
            let cursor = CursorBuilder::new(&self.blackbrane, CF_LOCK)
                .fill_cache(self.fill_cache)
                .mutant_search_mode(self.get_mutant_search_mode(true))
                .build()?;
            self.dagger_cursor = Some(cursor);
        }
        Ok(())
    }

    /// Return the first committed key for which `start_ts` equals to `ts`
    pub fn seek_ts(&mut self, ts: TimeStamp) -> Result<Option<Key>> {
        assert!(self.mutant_search_mode.is_some());
        self.create_write_cursor()?;

        let cursor = self.write_cursor.as_mut().unwrap();
        let mut ok = cursor.seek_to_first(&mut self.statistics.write);

        while ok {
            if WriteRef::parse(cursor.value(&mut self.statistics.write))?.start_ts == ts {
                return Ok(Some(
                    Key::from_encoded(cursor.key(&mut self.statistics.write).to_vec())
                        .truncate_ts()?,
                ));
            }
            ok = cursor.next(&mut self.statistics.write);
        }
        Ok(None)
    }

    /// SentinelSearch daggers that satisfies `filter(dagger)` returns true, from the given start key `start`.
    /// At most `limit` daggers will be returned. If `limit` is set to `0`, it means unlimited.
    ///
    /// The return type is `(daggers, is_remain)`. `is_remain` indicates whether there MAY be
    /// remaining daggers that can be mutant_searchned.
    pub fn mutant_search_daggers<F>(
        &mut self,
        start: Option<&Key>,
        end: Option<&Key>,
        filter: F,
        limit: usize,
    ) -> Result<(Vec<(Key, Dagger)>, bool)>
    where
        F: Fn(&Dagger) -> bool,
    {
        self.create_dagger_cursor()?;
        let cursor = self.dagger_cursor.as_mut().unwrap();
        let ok = match start {
            Some(x) => cursor.seek(x, &mut self.statistics.dagger)?,
            None => cursor.seek_to_first(&mut self.statistics.dagger),
        };
        if !ok {
            return Ok((vec![], false));
        }
        let mut daggers = Vec::with_capacity(limit);
        while cursor.valid()? {
            let key = Key::from_encoded_slice(cursor.key(&mut self.statistics.dagger));
            if let Some(end) = end {
                if key >= *end {
                    return Ok((daggers, false));
                }
            }

            let dagger = Dagger::parse(cursor.value(&mut self.statistics.dagger))?;
            if filter(&dagger) {
                daggers.push((key, dagger));
                if limit > 0 && daggers.len() == limit {
                    return Ok((daggers, true));
                }
            }
            cursor.next(&mut self.statistics.dagger);
        }
        self.statistics.dagger.processed_keys += daggers.len();
        // If we reach here, `cursor.valid()` is `false`, so there MUST be no more daggers.
        Ok((daggers, false))
    }

    pub fn mutant_search_keys(
        &mut self,
        mut start: Option<Key>,
        limit: usize,
    ) -> Result<(Vec<Key>, Option<Key>)> {
        let mut cursor = CursorBuilder::new(&self.blackbrane, CF_WRITE)
            .fill_cache(self.fill_cache)
            .mutant_search_mode(self.get_mutant_search_mode(false))
            .build()?;
        let mut keys = vec![];
        loop {
            let ok = match start {
                Some(ref x) => cursor.near_seek(x, &mut self.statistics.write)?,
                None => cursor.seek_to_first(&mut self.statistics.write),
            };
            if !ok {
                return Ok((keys, None));
            }
            if keys.len() >= limit {
                self.statistics.write.processed_keys += keys.len();
                resource_metering::record_read_keys(keys.len() as u32);
                return Ok((keys, start));
            }
            let key =
                Key::from_encoded(cursor.key(&mut self.statistics.write).to_vec()).truncate_ts()?;
            start = Some(key.clone().append_ts(TimeStamp::zero()));
            keys.push(key);
        }
    }

    // Get all Value of the given key in CF_DEFAULT
    pub fn mutant_search_values_in_default(&mut self, key: &Key) -> Result<Vec<(TimeStamp, Value)>> {
        self.create_data_cursor()?;
        let cursor = self.data_cursor.as_mut().unwrap();
        let mut ok = cursor.seek(key, &mut self.statistics.data)?;
        if !ok {
            return Ok(vec![]);
        }
        let mut v = vec![];
        while ok {
            let cur_key = cursor.key(&mut self.statistics.data);
            let ts = Key::decode_ts_from(cur_key)?;
            if Key::is_user_key_eq(cur_key, key.as_encoded()) {
                v.push((ts, cursor.value(&mut self.statistics.data).to_vec()));
            } else {
                break;
            }
            ok = cursor.next(&mut self.statistics.data);
        }
        Ok(v)
    }

    /// Read the old value for key for CDC.
    /// `prev_write` stands for the previous write record of the key
    /// it must be read in the caller and be passed in for optimization
    fn get_old_value(
        &mut self,
        key: &Key,
        start_ts: TimeStamp,
        prev_write_loaded: bool,
        prev_write: Option<Write>,
    ) -> Result<OldValue> {
        if prev_write_loaded && prev_write.is_none() {
            return Ok(OldValue::None);
        }
        if let Some(prev_write) = prev_write {
            if !prev_write
                .as_ref()
                .check_gc_fence_as_latest_version(start_ts)
            {
                return Ok(OldValue::None);
            }

            match prev_write.write_type {
                WriteType::Put => {
                    // For Put, there must be an old value either in its
                    // short value or in the default CF.
                    return Ok(match prev_write.short_value {
                        Some(value) => OldValue::Value { value },
                        None => OldValue::ValueTimeStamp {
                            start_ts: prev_write.start_ts,
                        },
                    });
                }
                WriteType::Delete => {
                    // For Delete, no old value.
                    return Ok(OldValue::None);
                }
                // For Rollback and Dagger, it's unknown whether there is a more
                // previous valid write. Call `get_write` to get a valid
                // previous write.
                WriteType::Rollback | WriteType::Dagger => (),
            }
        }
        Ok(match self.get_write(key, start_ts, Some(start_ts))? {
            Some(write) => match write.short_value {
                Some(value) => OldValue::Value { value },
                None => OldValue::ValueTimeStamp {
                    start_ts: write.start_ts,
                },
            },
            None => OldValue::None,
        })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::einsteindb::storage::fdbhikv::Modify;
    use crate::einsteindb::storage::epaxos::{tests::write, EpaxosReader, EpaxosTxn};
    use crate::einsteindb::storage::solitontxn::{
        acquire_pessimistic_dagger, cleanup, commit, gc, prewrite, CommitKind, TransactionKind,
        TransactionProperties,
    };
    use crate::einsteindb::storage::{einstein_merkle_tree, Testeinstein_merkle_treeBuilder};
    use concurrency_manager::ConcurrencyManager;
    use einstein_merkle_tree_rocks::properties::EpaxosPropertiesCollectorFactory;
    use einstein_merkle_tree_rocks::cocauset::DB;
    use einstein_merkle_tree_rocks::cocauset::{ColumnFamilyOptions, DBOptions};
    use einstein_merkle_tree_rocks::cocauset_util::CFOptions;
    use einstein_merkle_tree_rocks::{Compat, Rocksblackbrane};
    use einsteindb-gen::{IterOptions, Mutable, WriteBatch, WriteBatchExt};
    use einsteindb-gen::{ALL_CFS, CF_DEFAULT, CF_LOCK, CF_RAFT, CF_WRITE};
    use fdbhikvproto::fdbhikvrpcpb::{AssertionLevel, Context};
    use fdbhikvproto::metapb::{Peer, Region};
    use raftstore::store::Regionblackbrane;
    use std::ops::Bound;
    use std::sync::Arc;
    use std::u64;
    use solitontxn_types::{DaggerType, Mutation};

    pub struct Regioneinstein_merkle_tree {
        db: Arc<DB>,
        region: Region,
    }

    impl Regioneinstein_merkle_tree {
        pub fn new(db: &Arc<DB>, region: &Region) -> Regioneinstein_merkle_tree {
            Regioneinstein_merkle_tree {
                db: Arc::clone(db),
                region: region.clone(),
            }
        }

        pub fn blackbrane(&self) -> Regionblackbrane<Rocksblackbrane> {
            let db = self.db.c().clone();
            Regionblackbrane::<Rocksblackbrane>::from_cocauset(db, self.region.clone())
        }

        pub fn put(
            &mut self,
            pk: &[u8],
            start_ts: impl Into<TimeStamp>,
            commit_ts: impl Into<TimeStamp>,
        ) {
            let start_ts = start_ts.into();
            let m = Mutation::make_put(Key::from_cocauset(pk), vec![]);
            self.prewrite(m, pk, start_ts);
            self.commit(pk, start_ts, commit_ts);
        }

        pub fn dagger(
            &mut self,
            pk: &[u8],
            start_ts: impl Into<TimeStamp>,
            commit_ts: impl Into<TimeStamp>,
        ) {
            let start_ts = start_ts.into();
            let m = Mutation::make_dagger(Key::from_cocauset(pk));
            self.prewrite(m, pk, start_ts);
            self.commit(pk, start_ts, commit_ts);
        }

        pub fn delete(
            &mut self,
            pk: &[u8],
            start_ts: impl Into<TimeStamp>,
            commit_ts: impl Into<TimeStamp>,
        ) {
            let start_ts = start_ts.into();
            let m = Mutation::make_delete(Key::from_cocauset(pk));
            self.prewrite(m, pk, start_ts);
            self.commit(pk, start_ts, commit_ts);
        }

        pub fn solitontxn_props(
            start_ts: TimeStamp,
            primary: &[u8],
            pessimistic: bool,
        ) -> TransactionProperties<'_> {
            let kind = if pessimistic {
                TransactionKind::Pessimistic(TimeStamp::default())
            } else {
                TransactionKind::Optimistic(false)
            };

            TransactionProperties {
                start_ts,
                kind,
                commit_kind: CommitKind::TwoPc,
                primary,
                solitontxn_size: 0,
                dagger_ttl: 0,
                min_commit_ts: TimeStamp::default(),
                need_old_value: false,
                is_retry_request: false,
                assertion_level: AssertionLevel::Off,
            }
        }

        pub fn prewrite(&mut self, m: Mutation, pk: &[u8], start_ts: impl Into<TimeStamp>) {
            let snap = self.blackbrane();
            let start_ts = start_ts.into();
            let cm = ConcurrencyManager::new(start_ts);
            let mut solitontxn = EpaxosTxn::new(start_ts, cm);
            let mut reader = blackbraneReader::new(start_ts, snap, true);

            prewrite(
                &mut solitontxn,
                &mut reader,
                &Self::solitontxn_props(start_ts, pk, false),
                m,
                &None,
                false,
            )
            .unwrap();
            self.write(solitontxn.into_modifies());
        }

        pub fn prewrite_pessimistic_dagger(
            &mut self,
            m: Mutation,
            pk: &[u8],
            start_ts: impl Into<TimeStamp>,
        ) {
            let snap = self.blackbrane();
            let start_ts = start_ts.into();
            let cm = ConcurrencyManager::new(start_ts);
            let mut solitontxn = EpaxosTxn::new(start_ts, cm);
            let mut reader = blackbraneReader::new(start_ts, snap, true);

            prewrite(
                &mut solitontxn,
                &mut reader,
                &Self::solitontxn_props(start_ts, pk, true),
                m,
                &None,
                true,
            )
            .unwrap();
            self.write(solitontxn.into_modifies());
        }

        pub fn acquire_pessimistic_dagger(
            &mut self,
            k: Key,
            pk: &[u8],
            start_ts: impl Into<TimeStamp>,
            for_update_ts: impl Into<TimeStamp>,
        ) {
            let snap = self.blackbrane();
            let for_update_ts = for_update_ts.into();
            let cm = ConcurrencyManager::new(for_update_ts);
            let start_ts = start_ts.into();
            let mut solitontxn = EpaxosTxn::new(start_ts, cm);
            let mut reader = blackbraneReader::new(start_ts, snap, true);
            acquire_pessimistic_dagger(
                &mut solitontxn,
                &mut reader,
                k,
                pk,
                false,
                0,
                for_update_ts,
                false,
                false,
                TimeStamp::zero(),
                true,
            )
            .unwrap();
            self.write(solitontxn.into_modifies());
        }

        pub fn commit(
            &mut self,
            pk: &[u8],
            start_ts: impl Into<TimeStamp>,
            commit_ts: impl Into<TimeStamp>,
        ) {
            let snap = self.blackbrane();
            let start_ts = start_ts.into();
            let cm = ConcurrencyManager::new(start_ts);
            let mut solitontxn = EpaxosTxn::new(start_ts, cm);
            let mut reader = blackbraneReader::new(start_ts, snap, true);
            commit(&mut solitontxn, &mut reader, Key::from_cocauset(pk), commit_ts.into()).unwrap();
            self.write(solitontxn.into_modifies());
        }

        pub fn rollback(&mut self, pk: &[u8], start_ts: impl Into<TimeStamp>) {
            let snap = self.blackbrane();
            let start_ts = start_ts.into();
            let cm = ConcurrencyManager::new(start_ts);
            let mut solitontxn = EpaxosTxn::new(start_ts, cm);
            let mut reader = blackbraneReader::new(start_ts, snap, true);
            cleanup(
                &mut solitontxn,
                &mut reader,
                Key::from_cocauset(pk),
                TimeStamp::zero(),
                true,
            )
            .unwrap();
            self.write(solitontxn.into_modifies());
        }

        pub fn gc(&mut self, pk: &[u8], safe_point: impl Into<TimeStamp> + Copy) {
            let cm = ConcurrencyManager::new(safe_point.into());
            loop {
                let snap = self.blackbrane();
                let mut solitontxn = EpaxosTxn::new(safe_point.into(), cm.clone());
                let mut reader = EpaxosReader::new(snap, None, true);
                gc(&mut solitontxn, &mut reader, Key::from_cocauset(pk), safe_point.into()).unwrap();
                let modifies = solitontxn.into_modifies();
                if modifies.is_empty() {
                    return;
                }
                self.write(modifies);
            }
        }

        pub fn write(&mut self, modifies: Vec<Modify>) {
            let db = &self.db;
            let mut wb = db.c().write_batch();
            for rev in modifies {
                match rev {
                    Modify::Put(cf, k, v) => {
                        let k = keys::data_key(k.as_encoded());
                        wb.put_cf(cf, &k, &v).unwrap();
                    }
                    Modify::Delete(cf, k) => {
                        let k = keys::data_key(k.as_encoded());
                        wb.delete_cf(cf, &k).unwrap();
                    }
                    Modify::PessimisticDagger(k, dagger) => {
                        let k = keys::data_key(k.as_encoded());
                        let v = dagger.into_dagger().to_bytes();
                        wb.put_cf(CF_LOCK, &k, &v).unwrap();
                    }
                    Modify::DeleteRange(cf, k1, k2, notify_only) => {
                        if !notify_only {
                            let k1 = keys::data_key(k1.as_encoded());
                            let k2 = keys::data_key(k2.as_encoded());
                            wb.delete_range_cf(cf, &k1, &k2).unwrap();
                        }
                    }
                }
            }
            wb.write().unwrap();
        }

        pub fn flush(&mut self) {
            for cf in ALL_CFS {
                let cf = einstein_merkle_tree_rocks::util::get_cf_handle(&self.db, cf).unwrap();
                self.db.flush_cf(cf, true).unwrap();
            }
        }

        pub fn compact(&mut self) {
            for cf in ALL_CFS {
                let cf = einstein_merkle_tree_rocks::util::get_cf_handle(&self.db, cf).unwrap();
                self.db.compact_range_cf(cf, None, None);
            }
        }
    }

    pub fn open_db(path: &str, with_properties: bool) -> Arc<DB> {
        let db_opts = DBOptions::new();
        let mut cf_opts = ColumnFamilyOptions::new();
        cf_opts.set_write_buffer_size(32 * 1024 * 1024);
        if with_properties {
            cf_opts.add_table_properties_collector_factory(
                "einstfdbhikv.test-collector",
                EpaxosPropertiesCollectorFactory::default(),
            );
        }
        let cfs_opts = vec![
            CFOptions::new(CF_DEFAULT, ColumnFamilyOptions::new()),
            CFOptions::new(CF_RAFT, ColumnFamilyOptions::new()),
            CFOptions::new(CF_LOCK, ColumnFamilyOptions::new()),
            CFOptions::new(CF_WRITE, cf_opts),
        ];
        Arc::new(einstein_merkle_tree_rocks::cocauset_util::new_einstein_merkle_tree_opt(path, db_opts, cfs_opts).unwrap())
    }

    pub fn make_region(id: u64, start_key: Vec<u8>, end_key: Vec<u8>) -> Region {
        let mut peer = Peer::default();
        peer.set_id(id);
        peer.set_store_id(id);
        let mut region = Region::default();
        region.set_id(id);
        region.set_start_key(start_key);
        region.set_end_key(end_key);
        region.mut_peers().push(peer);
        region
    }

    #[test]
    fn test_ts_filter() {
        let path = tempfile::Builder::new()
            .prefix("test_ts_filter")
            .tempdir()
            .unwrap();
        let path = path.path().to_str().unwrap();
        let region = make_region(1, vec![0], vec![13]);

        let db = open_db(path, true);
        let mut einstein_merkle_tree = Regioneinstein_merkle_tree::new(&db, &region);

        einstein_merkle_tree.put(&[2], 1, 2);
        einstein_merkle_tree.put(&[4], 3, 4);
        einstein_merkle_tree.flush();
        einstein_merkle_tree.put(&[6], 5, 6);
        einstein_merkle_tree.put(&[8], 7, 8);
        einstein_merkle_tree.flush();
        einstein_merkle_tree.put(&[10], 9, 10);
        einstein_merkle_tree.put(&[12], 11, 12);
        einstein_merkle_tree.flush();

        let snap = Regionblackbrane::<Rocksblackbrane>::from_cocauset(db.c().clone(), region);

        let tests = vec![
            // set nothing.
            (
                Bound::Unbounded,
                Bound::Unbounded,
                vec![2u64, 4, 6, 8, 10, 12],
            ),
            // test set both hint_min_ts and hint_max_ts.
            (Bound::Included(6), Bound::Included(8), vec![6u64, 8]),
            (Bound::Excluded(5), Bound::Included(8), vec![6u64, 8]),
            (Bound::Included(6), Bound::Excluded(9), vec![6u64, 8]),
            (Bound::Excluded(5), Bound::Excluded(9), vec![6u64, 8]),
            // test set only hint_min_ts.
            (Bound::Included(10), Bound::Unbounded, vec![10u64, 12]),
            (Bound::Excluded(9), Bound::Unbounded, vec![10u64, 12]),
            // test set only hint_max_ts.
            (Bound::Unbounded, Bound::Included(7), vec![2u64, 4, 6, 8]),
            (Bound::Unbounded, Bound::Excluded(8), vec![2u64, 4, 6, 8]),
        ];

        for (_, &(min, max, ref res)) in tests.iter().enumerate() {
            let mut iopt = IterOptions::default();
            iopt.set_hint_min_ts(min);
            iopt.set_hint_max_ts(max);

            let mut iter = snap.iter_cf(CF_WRITE, iopt).unwrap();

            for (i, expect_ts) in res.iter().enumerate() {
                if i == 0 {
                    assert_eq!(iter.seek_to_first().unwrap(), true);
                } else {
                    assert_eq!(iter.next().unwrap(), true);
                }

                let ts = Key::decode_ts_from(iter.key()).unwrap();
                assert_eq!(ts.into_inner(), *expect_ts);
            }

            assert_eq!(iter.next().unwrap(), false);
        }
    }

    #[test]
    fn test_ts_filter_lost_delete() {
        let dir = tempfile::Builder::new()
            .prefix("test_ts_filter_lost_deletion")
            .tempdir()
            .unwrap();
        let path = dir.path().to_str().unwrap();
        let region = make_region(1, vec![0], vec![]);

        let db = open_db(path, true);
        let mut einstein_merkle_tree = Regioneinstein_merkle_tree::new(&db, &region);

        let key1 = &[1];
        einstein_merkle_tree.put(key1, 2, 3);
        einstein_merkle_tree.flush();
        einstein_merkle_tree.compact();

        // Delete key 1 commit ts@5 and GC@6
        // Put key 2 commit ts@7
        let key2 = &[2];
        einstein_merkle_tree.put(key2, 6, 7);
        einstein_merkle_tree.delete(key1, 4, 5);
        einstein_merkle_tree.gc(key1, 6);
        einstein_merkle_tree.flush();

        // SentinelSearch fdbhikv with ts filter [1, 6].
        let mut iopt = IterOptions::default();
        iopt.set_hint_min_ts(Bound::Included(1));
        iopt.set_hint_max_ts(Bound::Included(6));

        let snap = Regionblackbrane::<Rocksblackbrane>::from_cocauset(db.c().clone(), region);
        let mut iter = snap.iter_cf(CF_WRITE, iopt).unwrap();

        // Must not omit the latest deletion of key1 to prevent seeing outdated record.
        assert_eq!(iter.seek_to_first().unwrap(), true);
        assert_eq!(
            Key::from_encoded_slice(iter.key())
                .to_cocauset()
                .unwrap()
                .as_slice(),
            key2
        );
        assert_eq!(iter.next().unwrap(), false);
    }

    #[test]
    fn test_get_solitontxn_commit_record() {
        let path = tempfile::Builder::new()
            .prefix("_test_storage_epaxos_reader_get_solitontxn_commit_record")
            .tempdir()
            .unwrap();
        let path = path.path().to_str().unwrap();
        let region = make_region(1, vec![], vec![]);
        let db = open_db(path, true);
        let mut einstein_merkle_tree = Regioneinstein_merkle_tree::new(&db, &region);

        let (k, v) = (b"k", b"v");
        let m = Mutation::make_put(Key::from_cocauset(k), v.to_vec());
        einstein_merkle_tree.prewrite(m, k, 1);
        einstein_merkle_tree.commit(k, 1, 10);

        einstein_merkle_tree.rollback(k, 5);
        einstein_merkle_tree.rollback(k, 20);

        let m = Mutation::make_put(Key::from_cocauset(k), v.to_vec());
        einstein_merkle_tree.prewrite(m, k, 25);
        einstein_merkle_tree.commit(k, 25, 30);

        let m = Mutation::make_put(Key::from_cocauset(k), v.to_vec());
        einstein_merkle_tree.prewrite(m, k, 35);
        einstein_merkle_tree.commit(k, 35, 40);

        // Overlapped rollback on the commit record at 40.
        einstein_merkle_tree.rollback(k, 40);

        let m = Mutation::make_put(Key::from_cocauset(k), v.to_vec());
        einstein_merkle_tree.acquire_pessimistic_dagger(Key::from_cocauset(k), k, 45, 45);
        einstein_merkle_tree.prewrite_pessimistic_dagger(m, k, 45);
        einstein_merkle_tree.commit(k, 45, 50);

        let snap = Regionblackbrane::<Rocksblackbrane>::from_cocauset(db.c().clone(), region);
        let mut reader = EpaxosReader::new(snap, None, false);

        // Let's assume `50_45 PUT` means a commit version with start ts is 45 and commit ts
        // is 50.
        // Commit versions: [50_45 PUT, 45_40 PUT, 40_35 PUT, 30_25 PUT, 20_20 Rollback, 10_1 PUT, 5_5 Rollback].
        let key = Key::from_cocauset(k);
        let overlapped_write = reader
            .get_solitontxn_commit_record(&key, 55.into())
            .unwrap()
            .unwrap_none();
        assert!(overlapped_write.is_none());

        // When no such record is found but a record of another solitontxn has a write record with
        // its commit_ts equals to current start_ts, it
        let overlapped_write = reader
            .get_solitontxn_commit_record(&key, 50.into())
            .unwrap()
            .unwrap_none()
            .unwrap();
        assert_eq!(overlapped_write.write.start_ts, 45.into());
        assert_eq!(overlapped_write.write.write_type, WriteType::Put);

        let (commit_ts, write_type) = reader
            .get_solitontxn_commit_record(&key, 45.into())
            .unwrap()
            .unwrap_single_record();
        assert_eq!(commit_ts, 50.into());
        assert_eq!(write_type, WriteType::Put);

        let commit_ts = reader
            .get_solitontxn_commit_record(&key, 40.into())
            .unwrap()
            .unwrap_overlapped_rollback();
        assert_eq!(commit_ts, 40.into());

        let (commit_ts, write_type) = reader
            .get_solitontxn_commit_record(&key, 35.into())
            .unwrap()
            .unwrap_single_record();
        assert_eq!(commit_ts, 40.into());
        assert_eq!(write_type, WriteType::Put);

        let (commit_ts, write_type) = reader
            .get_solitontxn_commit_record(&key, 25.into())
            .unwrap()
            .unwrap_single_record();
        assert_eq!(commit_ts, 30.into());
        assert_eq!(write_type, WriteType::Put);

        let (commit_ts, write_type) = reader
            .get_solitontxn_commit_record(&key, 20.into())
            .unwrap()
            .unwrap_single_record();
        assert_eq!(commit_ts, 20.into());
        assert_eq!(write_type, WriteType::Rollback);

        let (commit_ts, write_type) = reader
            .get_solitontxn_commit_record(&key, 1.into())
            .unwrap()
            .unwrap_single_record();
        assert_eq!(commit_ts, 10.into());
        assert_eq!(write_type, WriteType::Put);

        let (commit_ts, write_type) = reader
            .get_solitontxn_commit_record(&key, 5.into())
            .unwrap()
            .unwrap_single_record();
        assert_eq!(commit_ts, 5.into());
        assert_eq!(write_type, WriteType::Rollback);

        let seek_old = reader.statistics.write.seek;
        let next_old = reader.statistics.write.next;
        assert!(
            !reader
                .get_solitontxn_commit_record(&key, 30.into())
                .unwrap()
                .exist()
        );
        let seek_new = reader.statistics.write.seek;
        let next_new = reader.statistics.write.next;

        // `get_solitontxn_commit_record(&key, 30)` stopped at `30_25 PUT`.
        assert_eq!(seek_new - seek_old, 1);
        assert_eq!(next_new - next_old, 2);
    }

    #[test]
    fn test_get_solitontxn_commit_record_of_pessimistic_solitontxn() {
        let path = tempfile::Builder::new()
            .prefix("_test_storage_epaxos_reader_get_solitontxn_commit_record_of_pessimistic_solitontxn")
            .tempdir()
            .unwrap();
        let path = path.path().to_str().unwrap();
        let region = make_region(1, vec![], vec![]);
        let db = open_db(path, true);
        let mut einstein_merkle_tree = Regioneinstein_merkle_tree::new(&db, &region);

        let (k, v) = (b"k", b"v");
        let key = Key::from_cocauset(k);
        let m = Mutation::make_put(key.clone(), v.to_vec());

        // solitontxn: start_ts = 2, commit_ts = 3
        einstein_merkle_tree.acquire_pessimistic_dagger(key.clone(), k, 2, 2);
        einstein_merkle_tree.prewrite_pessimistic_dagger(m.clone(), k, 2);
        einstein_merkle_tree.commit(k, 2, 3);
        // solitontxn: start_ts = 1, commit_ts = 4
        einstein_merkle_tree.acquire_pessimistic_dagger(key.clone(), k, 1, 3);
        einstein_merkle_tree.prewrite_pessimistic_dagger(m, k, 1);
        einstein_merkle_tree.commit(k, 1, 4);

        let snap = Regionblackbrane::<Rocksblackbrane>::from_cocauset(db.c().clone(), region);
        let mut reader = EpaxosReader::new(snap, None, false);

        let (commit_ts, write_type) = reader
            .get_solitontxn_commit_record(&key, 2.into())
            .unwrap()
            .unwrap_single_record();
        assert_eq!(commit_ts, 3.into());
        assert_eq!(write_type, WriteType::Put);

        let (commit_ts, write_type) = reader
            .get_solitontxn_commit_record(&key, 1.into())
            .unwrap()
            .unwrap_single_record();
        assert_eq!(commit_ts, 4.into());
        assert_eq!(write_type, WriteType::Put);
    }

    #[test]
    fn test_seek_write() {
        let path = tempfile::Builder::new()
            .prefix("_test_storage_epaxos_reader_seek_write")
            .tempdir()
            .unwrap();
        let path = path.path().to_str().unwrap();
        let region = make_region(1, vec![], vec![]);
        let db = open_db(path, true);
        let mut einstein_merkle_tree = Regioneinstein_merkle_tree::new(&db, &region);

        let (k, v) = (b"k", b"v");
        let m = Mutation::make_put(Key::from_cocauset(k), v.to_vec());
        einstein_merkle_tree.prewrite(m.clone(), k, 1);
        einstein_merkle_tree.commit(k, 1, 5);

        einstein_merkle_tree.write(vec![
            Modify::Put(
                CF_WRITE,
                Key::from_cocauset(k).append_ts(TimeStamp::new(3)),
                vec![b'R', 3],
            ),
            Modify::Put(
                CF_WRITE,
                Key::from_cocauset(k).append_ts(TimeStamp::new(7)),
                vec![b'R', 7],
            ),
        ]);

        einstein_merkle_tree.prewrite(m.clone(), k, 15);
        einstein_merkle_tree.commit(k, 15, 17);

        // Timestamp overlap with the previous transaction.
        einstein_merkle_tree.acquire_pessimistic_dagger(Key::from_cocauset(k), k, 10, 18);
        einstein_merkle_tree.prewrite_pessimistic_dagger(Mutation::make_dagger(Key::from_cocauset(k)), k, 10);
        einstein_merkle_tree.commit(k, 10, 20);

        einstein_merkle_tree.prewrite(m, k, 23);
        einstein_merkle_tree.commit(k, 23, 25);

        // Let's assume `2_1 PUT` means a commit version with start ts is 1 and commit ts
        // is 2.
        // Commit versions: [25_23 PUT, 20_10 PUT, 17_15 PUT, 7_7 Rollback, 5_1 PUT, 3_3 Rollback].
        let snap = Regionblackbrane::<Rocksblackbrane>::from_cocauset(db.c().clone(), region.clone());
        let mut reader = EpaxosReader::new(snap, None, false);

        let k = Key::from_cocauset(k);
        let (commit_ts, write) = reader.seek_write(&k, 30.into()).unwrap().unwrap();
        assert_eq!(commit_ts, 25.into());
        assert_eq!(
            write,
            Write::new(WriteType::Put, 23.into(), Some(v.to_vec()))
        );
        assert_eq!(reader.statistics.write.seek, 1);
        assert_eq!(reader.statistics.write.next, 0);

        let (commit_ts, write) = reader.seek_write(&k, 25.into()).unwrap().unwrap();
        assert_eq!(commit_ts, 25.into());
        assert_eq!(
            write,
            Write::new(WriteType::Put, 23.into(), Some(v.to_vec()))
        );
        assert_eq!(reader.statistics.write.seek, 1);
        assert_eq!(reader.statistics.write.next, 0);

        let (commit_ts, write) = reader.seek_write(&k, 20.into()).unwrap().unwrap();
        assert_eq!(commit_ts, 20.into());
        assert_eq!(write, Write::new(WriteType::Dagger, 10.into(), None));
        assert_eq!(reader.statistics.write.seek, 1);
        assert_eq!(reader.statistics.write.next, 1);

        let (commit_ts, write) = reader.seek_write(&k, 19.into()).unwrap().unwrap();
        assert_eq!(commit_ts, 17.into());
        assert_eq!(
            write,
            Write::new(WriteType::Put, 15.into(), Some(v.to_vec()))
        );
        assert_eq!(reader.statistics.write.seek, 1);
        assert_eq!(reader.statistics.write.next, 2);

        let (commit_ts, write) = reader.seek_write(&k, 3.into()).unwrap().unwrap();
        assert_eq!(commit_ts, 3.into());
        assert_eq!(write, Write::new_rollback(3.into(), false));
        assert_eq!(reader.statistics.write.seek, 1);
        assert_eq!(reader.statistics.write.next, 5);

        let (commit_ts, write) = reader.seek_write(&k, 16.into()).unwrap().unwrap();
        assert_eq!(commit_ts, 7.into());
        assert_eq!(write, Write::new_rollback(7.into(), false));
        assert_eq!(reader.statistics.write.seek, 1);
        assert_eq!(reader.statistics.write.next, 6);
        assert_eq!(reader.statistics.write.prev, 3);

        let (commit_ts, write) = reader.seek_write(&k, 6.into()).unwrap().unwrap();
        assert_eq!(commit_ts, 5.into());
        assert_eq!(
            write,
            Write::new(WriteType::Put, 1.into(), Some(v.to_vec()))
        );
        assert_eq!(reader.statistics.write.seek, 1);
        assert_eq!(reader.statistics.write.next, 7);
        assert_eq!(reader.statistics.write.prev, 3);

        assert!(reader.seek_write(&k, 2.into()).unwrap().is_none());
        assert_eq!(reader.statistics.write.seek, 1);
        assert_eq!(reader.statistics.write.next, 9);
        assert_eq!(reader.statistics.write.prev, 3);

        // Test seek_write should not see the next key.
        let (k2, v2) = (b"k2", b"v2");
        let m2 = Mutation::make_put(Key::from_cocauset(k2), v2.to_vec());
        einstein_merkle_tree.prewrite(m2, k2, 1);
        einstein_merkle_tree.commit(k2, 1, 2);

        let snap = Regionblackbrane::<Rocksblackbrane>::from_cocauset(db.c().clone(), region);
        let mut reader = EpaxosReader::new(snap, None, false);

        let (commit_ts, write) = reader
            .seek_write(&Key::from_cocauset(k2), 3.into())
            .unwrap()
            .unwrap();
        assert_eq!(commit_ts, 2.into());
        assert_eq!(
            write,
            Write::new(WriteType::Put, 1.into(), Some(v2.to_vec()))
        );
        assert_eq!(reader.statistics.write.seek, 1);
        assert_eq!(reader.statistics.write.next, 0);

        // Should seek for another key.
        assert!(reader.seek_write(&k, 2.into()).unwrap().is_none());
        assert_eq!(reader.statistics.write.seek, 2);
        assert_eq!(reader.statistics.write.next, 0);

        // Test seek_write touches region's end.
        let region1 = make_region(1, vec![], Key::from_cocauset(b"k1").into_encoded());
        let snap = Regionblackbrane::<Rocksblackbrane>::from_cocauset(db.c().clone(), region1);
        let mut reader = EpaxosReader::new(snap, None, false);

        assert!(reader.seek_write(&k, 2.into()).unwrap().is_none());
    }

    #[test]
    fn test_get_write() {
        let path = tempfile::Builder::new()
            .prefix("_test_storage_epaxos_reader_get_write")
            .tempdir()
            .unwrap();
        let path = path.path().to_str().unwrap();
        let region = make_region(1, vec![], vec![]);
        let db = open_db(path, true);
        let mut einstein_merkle_tree = Regioneinstein_merkle_tree::new(&db, &region);

        let (k, v) = (b"k", b"v");
        let m = Mutation::make_put(Key::from_cocauset(k), v.to_vec());
        einstein_merkle_tree.prewrite(m, k, 1);
        einstein_merkle_tree.commit(k, 1, 2);

        einstein_merkle_tree.rollback(k, 5);

        einstein_merkle_tree.dagger(k, 6, 7);

        einstein_merkle_tree.delete(k, 8, 9);

        let m = Mutation::make_put(Key::from_cocauset(k), v.to_vec());
        einstein_merkle_tree.prewrite(m, k, 12);
        einstein_merkle_tree.commit(k, 12, 14);

        let m = Mutation::make_dagger(Key::from_cocauset(k));
        einstein_merkle_tree.acquire_pessimistic_dagger(Key::from_cocauset(k), k, 13, 15);
        einstein_merkle_tree.prewrite_pessimistic_dagger(m, k, 13);
        einstein_merkle_tree.commit(k, 13, 15);

        let m = Mutation::make_put(Key::from_cocauset(k), v.to_vec());
        einstein_merkle_tree.acquire_pessimistic_dagger(Key::from_cocauset(k), k, 18, 18);
        einstein_merkle_tree.prewrite_pessimistic_dagger(m, k, 18);
        einstein_merkle_tree.commit(k, 18, 20);

        let m = Mutation::make_dagger(Key::from_cocauset(k));
        einstein_merkle_tree.acquire_pessimistic_dagger(Key::from_cocauset(k), k, 17, 21);
        einstein_merkle_tree.prewrite_pessimistic_dagger(m, k, 17);
        einstein_merkle_tree.commit(k, 17, 21);

        let m = Mutation::make_put(Key::from_cocauset(k), v.to_vec());
        einstein_merkle_tree.prewrite(m, k, 24);

        let snap = Regionblackbrane::<Rocksblackbrane>::from_cocauset(db.c().clone(), region);
        let mut reader = EpaxosReader::new(snap, None, false);

        // Let's assume `2_1 PUT` means a commit version with start ts is 1 and commit ts
        // is 2.
        // Commit versions: [21_17 LOCK, 20_18 PUT, 15_13 LOCK, 14_12 PUT, 9_8 DELETE, 7_6 LOCK,
        //                   5_5 Rollback, 2_1 PUT].
        let key = Key::from_cocauset(k);

        assert!(reader.get_write(&key, 1.into(), None).unwrap().is_none());

        let write = reader.get_write(&key, 2.into(), None).unwrap().unwrap();
        assert_eq!(write.write_type, WriteType::Put);
        assert_eq!(write.start_ts, 1.into());

        let write = reader.get_write(&key, 5.into(), None).unwrap().unwrap();
        assert_eq!(write.write_type, WriteType::Put);
        assert_eq!(write.start_ts, 1.into());

        let write = reader.get_write(&key, 7.into(), None).unwrap().unwrap();
        assert_eq!(write.write_type, WriteType::Put);
        assert_eq!(write.start_ts, 1.into());

        assert!(reader.get_write(&key, 9.into(), None).unwrap().is_none());

        let write = reader.get_write(&key, 14.into(), None).unwrap().unwrap();
        assert_eq!(write.write_type, WriteType::Put);
        assert_eq!(write.start_ts, 12.into());

        let write = reader.get_write(&key, 16.into(), None).unwrap().unwrap();
        assert_eq!(write.write_type, WriteType::Put);
        assert_eq!(write.start_ts, 12.into());

        let write = reader.get_write(&key, 20.into(), None).unwrap().unwrap();
        assert_eq!(write.write_type, WriteType::Put);
        assert_eq!(write.start_ts, 18.into());

        let write = reader.get_write(&key, 24.into(), None).unwrap().unwrap();
        assert_eq!(write.write_type, WriteType::Put);
        assert_eq!(write.start_ts, 18.into());

        assert!(
            reader
                .get_write(&Key::from_cocauset(b"j"), 100.into(), None)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_mutant_search_daggers() {
        let path = tempfile::Builder::new()
            .prefix("_test_storage_epaxos_reader_mutant_search_daggers")
            .tempdir()
            .unwrap();
        let path = path.path().to_str().unwrap();
        let region = make_region(1, vec![], vec![]);
        let db = open_db(path, true);
        let mut einstein_merkle_tree = Regioneinstein_merkle_tree::new(&db, &region);

        // Put some daggers to the db.
        einstein_merkle_tree.prewrite(
            Mutation::make_put(Key::from_cocauset(b"k1"), b"v1".to_vec()),
            b"k1",
            5,
        );
        einstein_merkle_tree.prewrite(
            Mutation::make_put(Key::from_cocauset(b"k2"), b"v2".to_vec()),
            b"k1",
            10,
        );
        einstein_merkle_tree.prewrite(Mutation::make_delete(Key::from_cocauset(b"k3")), b"k1", 10);
        einstein_merkle_tree.prewrite(Mutation::make_dagger(Key::from_cocauset(b"k3\x00")), b"k1", 10);
        einstein_merkle_tree.prewrite(Mutation::make_delete(Key::from_cocauset(b"k4")), b"k1", 12);
        einstein_merkle_tree.acquire_pessimistic_dagger(Key::from_cocauset(b"k5"), b"k1", 10, 12);
        einstein_merkle_tree.acquire_pessimistic_dagger(Key::from_cocauset(b"k6"), b"k1", 12, 12);

        // All daggers whose ts <= 10.
        let visible_daggers: Vec<_> = vec![
            // key, dagger_type, short_value, ts, for_update_ts
            (
                b"k1".to_vec(),
                DaggerType::Put,
                Some(b"v1".to_vec()),
                5.into(),
                TimeStamp::zero(),
            ),
            (
                b"k2".to_vec(),
                DaggerType::Put,
                Some(b"v2".to_vec()),
                10.into(),
                TimeStamp::zero(),
            ),
            (
                b"k3".to_vec(),
                DaggerType::Delete,
                None,
                10.into(),
                TimeStamp::zero(),
            ),
            (
                b"k3\x00".to_vec(),
                DaggerType::Dagger,
                None,
                10.into(),
                TimeStamp::zero(),
            ),
            (
                b"k5".to_vec(),
                DaggerType::Pessimistic,
                None,
                10.into(),
                12.into(),
            ),
        ]
        .into_iter()
        .map(|(k, dagger_type, short_value, ts, for_update_ts)| {
            (
                Key::from_cocauset(&k),
                Dagger::new(
                    dagger_type,
                    b"k1".to_vec(),
                    ts,
                    0,
                    short_value,
                    for_update_ts,
                    0,
                    TimeStamp::zero(),
                ),
            )
        })
        .collect();

        // Creates a reader and mutant_search daggers,
        let check_mutant_search_dagger = |start_key: Option<Key>,
                               end_key: Option<Key>,
                               limit,
                               expect_res: &[_],
                               expect_is_remain| {
            let snap = Regionblackbrane::<Rocksblackbrane>::from_cocauset(db.c().clone(), region.clone());
            let mut reader = EpaxosReader::new(snap, None, false);
            let res = reader
                .mutant_search_daggers(
                    start_key.as_ref(),
                    end_key.as_ref(),
                    |l| l.ts <= 10.into(),
                    limit,
                )
                .unwrap();
            assert_eq!(res.0, expect_res);
            assert_eq!(res.1, expect_is_remain);
        };

        check_mutant_search_dagger(None, None, 6, &visible_daggers, false);
        check_mutant_search_dagger(None, None, 5, &visible_daggers, true);
        check_mutant_search_dagger(None, None, 4, &visible_daggers[0..4], true);
        check_mutant_search_dagger(
            Some(Key::from_cocauset(b"k2")),
            None,
            3,
            &visible_daggers[1..4],
            true,
        );
        check_mutant_search_dagger(
            Some(Key::from_cocauset(b"k3\x00")),
            None,
            1,
            &visible_daggers[3..4],
            true,
        );
        check_mutant_search_dagger(
            Some(Key::from_cocauset(b"k3\x00")),
            None,
            10,
            &visible_daggers[3..],
            false,
        );
        // limit = 0 means unlimited.
        check_mutant_search_dagger(None, None, 0, &visible_daggers, false);
        // Test mutant_searchning with limited end_key
        check_mutant_search_dagger(
            None,
            Some(Key::from_cocauset(b"k3")),
            0,
            &visible_daggers[..2],
            false,
        );
        check_mutant_search_dagger(
            None,
            Some(Key::from_cocauset(b"k3\x00")),
            0,
            &visible_daggers[..3],
            false,
        );
        check_mutant_search_dagger(
            None,
            Some(Key::from_cocauset(b"k3\x00")),
            3,
            &visible_daggers[..3],
            true,
        );
        check_mutant_search_dagger(
            None,
            Some(Key::from_cocauset(b"k3\x00")),
            2,
            &visible_daggers[..2],
            true,
        );
    }

    #[test]
    fn test_load_data() {
        let path = tempfile::Builder::new()
            .prefix("_test_storage_epaxos_reader_load_data")
            .tempdir()
            .unwrap();
        let path = path.path().to_str().unwrap();
        let region = make_region(1, vec![], vec![]);
        let db = open_db(path, true);
        let mut einstein_merkle_tree = Regioneinstein_merkle_tree::new(&db, &region);

        let (k, short_value, long_value) = (
            b"k",
            b"v",
            "v".repeat(solitontxn_types::SHORT_VALUE_MAX_LEN + 1).into_bytes(),
        );

        struct Case {
            expected: Result<Value>,

            // modifies to put into the einstein_merkle_tree
            modifies: Vec<Modify>,
            // these are used to construct the epaxos reader
            mutant_search_mode: Option<SentinelSearchMode>,
            key: Key,
            write: Write,
        }

        let cases = vec![
            Case {
                // write has short_value
                expected: Ok(short_value.to_vec()),

                modifies: vec![Modify::Put(
                    CF_DEFAULT,
                    Key::from_cocauset(k).append_ts(TimeStamp::new(1)),
                    vec![],
                )],
                mutant_search_mode: None,
                key: Key::from_cocauset(k),
                write: Write::new(
                    WriteType::Put,
                    TimeStamp::new(1),
                    Some(short_value.to_vec()),
                ),
            },
            Case {
                // write has no short_value, the reader has a cursor, got something
                expected: Ok(long_value.to_vec()),
                modifies: vec![Modify::Put(
                    CF_DEFAULT,
                    Key::from_cocauset(k).append_ts(TimeStamp::new(2)),
                    long_value.to_vec(),
                )],
                mutant_search_mode: Some(SentinelSearchMode::Forward),
                key: Key::from_cocauset(k),
                write: Write::new(WriteType::Put, TimeStamp::new(2), None),
            },
            Case {
                // write has no short_value, the reader has a cursor, got nothing
                expected: Err(default_not_found_error(k.to_vec(), "get")),
                modifies: vec![Modify::Put(
                    CF_WRITE,
                    Key::from_cocauset(k).append_ts(TimeStamp::new(1)),
                    Write::new(WriteType::Put, TimeStamp::new(1), None)
                        .as_ref()
                        .to_bytes(),
                )],
                mutant_search_mode: Some(SentinelSearchMode::Forward),
                key: Key::from_cocauset(k),
                write: Write::new(WriteType::Put, TimeStamp::new(3), None),
            },
            Case {
                // write has no short_value, the reader has no cursor, got something
                expected: Ok(long_value.to_vec()),
                modifies: vec![Modify::Put(
                    CF_DEFAULT,
                    Key::from_cocauset(k).append_ts(TimeStamp::new(4)),
                    long_value.to_vec(),
                )],
                mutant_search_mode: None,
                key: Key::from_cocauset(k),
                write: Write::new(WriteType::Put, TimeStamp::new(4), None),
            },
            Case {
                // write has no short_value, the reader has no cursor, got nothing
                expected: Err(default_not_found_error(k.to_vec(), "get")),
                modifies: vec![],
                mutant_search_mode: None,
                key: Key::from_cocauset(k),
                write: Write::new(WriteType::Put, TimeStamp::new(5), None),
            },
        ];

        for case in cases {
            einstein_merkle_tree.write(case.modifies);
            let snap = Regionblackbrane::<Rocksblackbrane>::from_cocauset(db.c().clone(), region.clone());
            let mut reader = EpaxosReader::new(snap, case.mutant_search_mode, false);
            let result = reader.load_data(&case.key, case.write);
            assert_eq!(format!("{:?}", result), format!("{:?}", case.expected));
        }
    }

    #[test]
    fn test_get() {
        let path = tempfile::Builder::new()
            .prefix("_test_storage_epaxos_reader_get")
            .tempdir()
            .unwrap();
        let path = path.path().to_str().unwrap();
        let region = make_region(1, vec![], vec![]);
        let db = open_db(path, true);
        let mut einstein_merkle_tree = Regioneinstein_merkle_tree::new(&db, &region);

        let (k, long_value) = (
            b"k",
            "v".repeat(solitontxn_types::SHORT_VALUE_MAX_LEN + 1).into_bytes(),
        );

        struct Case {
            expected: Result<Option<Value>>,
            // modifies to put into the einstein_merkle_tree
            modifies: Vec<Modify>,
            // arguments to do the function call
            key: Key,
            ts: TimeStamp,
            gc_fence_limit: Option<TimeStamp>,
        }

        let cases = vec![
            Case {
                // no write for `key` at `ts` exists
                expected: Ok(None),
                modifies: vec![Modify::Delete(
                    CF_DEFAULT,
                    Key::from_cocauset(k).append_ts(TimeStamp::new(1)),
                )],
                key: Key::from_cocauset(k),
                ts: TimeStamp::new(1),
                gc_fence_limit: None,
            },
            Case {
                // some write for `key` at `ts` exists, load data return Err
                // todo: "some write for `key` at `ts` exists" should be checked by `test_get_write`
                // "load data return Err" is checked by test_load_data
                expected: Err(default_not_found_error(k.to_vec(), "get")),
                modifies: vec![Modify::Put(
                    CF_WRITE,
                    Key::from_cocauset(k).append_ts(TimeStamp::new(2)),
                    Write::new(WriteType::Put, TimeStamp::new(2), None)
                        .as_ref()
                        .to_bytes(),
                )],
                key: Key::from_cocauset(k),
                ts: TimeStamp::new(3),
                gc_fence_limit: None,
            },
            Case {
                // some write for `key` at `ts` exists, load data success
                // todo: "some write for `key` at `ts` exists" should be checked by `test_get_write`
                // "load data success" is checked by test_load_data
                expected: Ok(Some(long_value.to_vec())),
                modifies: vec![
                    Modify::Put(
                        CF_WRITE,
                        Key::from_cocauset(k).append_ts(TimeStamp::new(4)),
                        Write::new(WriteType::Put, TimeStamp::new(4), None)
                            .as_ref()
                            .to_bytes(),
                    ),
                    Modify::Put(
                        CF_DEFAULT,
                        Key::from_cocauset(k).append_ts(TimeStamp::new(4)),
                        long_value,
                    ),
                ],
                key: Key::from_cocauset(k),
                ts: TimeStamp::new(5),
                gc_fence_limit: None,
            },
        ];

        for case in cases {
            einstein_merkle_tree.write(case.modifies);
            let snap = Regionblackbrane::<Rocksblackbrane>::from_cocauset(db.c().clone(), region.clone());
            let mut reader = EpaxosReader::new(snap, None, false);
            let result = reader.get(&case.key, case.ts, case.gc_fence_limit);
            assert_eq!(format!("{:?}", result), format!("{:?}", case.expected));
        }
    }

    #[test]
    fn test_get_old_value() {
        struct Case {
            expected: OldValue,

            // (write_record, put_ts)
            // all data to write to the einstein_merkle_tree
            // current write_cursor will be on the last record in `written`
            // which also means prev_write is `Write` in the record
            written: Vec<(Write, TimeStamp)>,
        }
        let cases = vec![
            // prev_write is None
            Case {
                expected: OldValue::None,
                written: vec![],
            },
            // prev_write is Rollback, and there exists a more previous valid write
            Case {
                expected: OldValue::ValueTimeStamp {
                    start_ts: TimeStamp::new(4),
                },

                written: vec![
                    (
                        Write::new(WriteType::Put, TimeStamp::new(4), None),
                        TimeStamp::new(6),
                    ),
                    (
                        Write::new(WriteType::Rollback, TimeStamp::new(5), None),
                        TimeStamp::new(7),
                    ),
                ],
            },
            Case {
                expected: OldValue::Value {
                    value: b"v".to_vec(),
                },

                written: vec![
                    (
                        Write::new(WriteType::Put, TimeStamp::new(4), Some(b"v".to_vec())),
                        TimeStamp::new(6),
                    ),
                    (
                        Write::new(WriteType::Rollback, TimeStamp::new(5), None),
                        TimeStamp::new(7),
                    ),
                ],
            },
            // prev_write is Rollback, and there isn't a more previous valid write
            Case {
                expected: OldValue::None,
                written: vec![(
                    Write::new(WriteType::Rollback, TimeStamp::new(5), None),
                    TimeStamp::new(6),
                )],
            },
            // prev_write is Dagger, and there exists a more previous valid write
            Case {
                expected: OldValue::ValueTimeStamp {
                    start_ts: TimeStamp::new(3),
                },

                written: vec![
                    (
                        Write::new(WriteType::Put, TimeStamp::new(3), None),
                        TimeStamp::new(6),
                    ),
                    (
                        Write::new(WriteType::Dagger, TimeStamp::new(5), None),
                        TimeStamp::new(7),
                    ),
                ],
            },
            // prev_write is Dagger, and there isn't a more previous valid write
            Case {
                expected: OldValue::None,
                written: vec![(
                    Write::new(WriteType::Dagger, TimeStamp::new(5), None),
                    TimeStamp::new(6),
                )],
            },
            // prev_write is not Rollback or Dagger, check_gc_fence_as_latest_version is true
            Case {
                expected: OldValue::ValueTimeStamp {
                    start_ts: TimeStamp::new(7),
                },
                written: vec![(
                    Write::new(WriteType::Put, TimeStamp::new(7), None)
                        .set_overlapped_rollback(true, Some(27.into())),
                    TimeStamp::new(5),
                )],
            },
            // prev_write is not Rollback or Dagger, check_gc_fence_as_latest_version is false
            Case {
                expected: OldValue::None,
                written: vec![(
                    Write::new(WriteType::Put, TimeStamp::new(4), None)
                        .set_overlapped_rollback(true, Some(3.into())),
                    TimeStamp::new(5),
                )],
            },
            // prev_write is Delete, check_gc_fence_as_latest_version is true
            Case {
                expected: OldValue::None,
                written: vec![
                    (
                        Write::new(WriteType::Put, TimeStamp::new(3), None),
                        TimeStamp::new(6),
                    ),
                    (
                        Write::new(WriteType::Delete, TimeStamp::new(7), None),
                        TimeStamp::new(8),
                    ),
                ],
            },
            // prev_write is Delete, check_gc_fence_as_latest_version is false
            Case {
                expected: OldValue::None,
                written: vec![
                    (
                        Write::new(WriteType::Put, TimeStamp::new(3), None),
                        TimeStamp::new(6),
                    ),
                    (
                        Write::new(WriteType::Delete, TimeStamp::new(7), None)
                            .set_overlapped_rollback(true, Some(6.into())),
                        TimeStamp::new(8),
                    ),
                ],
            },
        ];
        for (i, case) in cases.into_iter().enumerate() {
            let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
            let cm = ConcurrencyManager::new(42.into());
            let mut solitontxn = EpaxosTxn::new(TimeStamp::new(10), cm.clone());
            for (write_record, put_ts) in case.written.iter() {
                solitontxn.put_write(
                    Key::from_cocauset(b"a"),
                    *put_ts,
                    write_record.as_ref().to_bytes(),
                );
            }
            write(&einstein_merkle_tree, &Context::default(), solitontxn.into_modifies());
            let blackbrane = einstein_merkle_tree.blackbrane(Default::default()).unwrap();
            let mut reader = EpaxosReader::new(blackbrane, None, true);
            if !case.written.is_empty() {
                let prev_write = reader
                    .seek_write(&Key::from_cocauset(b"a"), case.written.last().unwrap().1)
                    .unwrap()
                    .map(|w| w.1);
                let prev_write_loaded = true;
                let result = reader
                    .get_old_value(
                        &Key::from_cocauset(b"a"),
                        TimeStamp::new(25),
                        prev_write_loaded,
                        prev_write,
                    )
                    .unwrap();
                assert_eq!(result, case.expected, "case #{}", i);
            }
        }

        // Must return Oldvalue::None when prev_write_loaded is true and prev_write is None.
        let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
        let blackbrane = einstein_merkle_tree.blackbrane(Default::default()).unwrap();
        let mut reader = EpaxosReader::new(blackbrane, None, true);
        let prev_write_loaded = true;
        let prev_write = None;
        let result = reader
            .get_old_value(
                &Key::from_cocauset(b"a"),
                TimeStamp::new(25),
                prev_write_loaded,
                prev_write,
            )
            .unwrap();
        assert_eq!(result, OldValue::None);
    }

    #[test]
    fn test_reader_prefix_seek() {
        let dir = tempfile::TempDir::new().unwrap();
        let builder = Testeinstein_merkle_treeBuilder::new().path(dir.path());
        let db = builder.build().unwrap().fdbhikv_einstein_merkle_tree().get_sync_db();
        let cf = einstein_merkle_tree_rocks::util::get_cf_handle(&db, CF_WRITE).unwrap();

        let region = make_region(1, vec![], vec![]);
        let mut einstein_merkle_tree = Regioneinstein_merkle_tree::new(&db, &region);

        // Put some tombstones into the DB.
        for i in 1..100 {
            let commit_ts = (i * 2 + 1).into();
            let mut k = vec![b'z'];
            k.extend_from_slice(Key::from_cocauset(b"k1").append_ts(commit_ts).as_encoded());
            use einstein_merkle_tree_rocks::cocauset::Writable;
            einstein_merkle_tree.db.delete_cf(cf, &k).unwrap();
        }
        einstein_merkle_tree.flush();

        #[allow(clippy::useless_vec)]
        for (k, mutant_search_mode, tombstones) in vec![
            (b"k0", Some(SentinelSearchMode::Forward), 99),
            (b"k0", None, 0),
            (b"k1", Some(SentinelSearchMode::Forward), 99),
            (b"k1", None, 99),
            (b"k2", Some(SentinelSearchMode::Forward), 0),
            (b"k2", None, 0),
        ] {
            let mut reader = EpaxosReader::new(einstein_merkle_tree.blackbrane(), mutant_search_mode, false);
            let (k, ts) = (Key::from_cocauset(k), 199.into());
            reader.seek_write(&k, ts).unwrap();
            assert_eq!(reader.statistics.write.seek_tombstone, tombstones);
        }
    }
}
