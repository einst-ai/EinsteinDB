// Copyright 2020 EinsteinDB Project Authors. Licensed under Apache-2.0.

// #[PerformanceCriticalPath]
//! Functionality for handling optimistic and pessimistic prewrites. These are separate commands
//! (although maybe they shouldn't be since there is only one protobuf), but
//! handling of the commands is similar. We therefore have a single type (Prewriter) to handle both
//! kinds of prewrite.

use crate::einsteindb::storage::{
    fdbhikv::WriteData,
    dagger_manager::DaggerManager,
    epaxos::{
        has_data_in_range, Error as EpaxosError, ErrorInner as EpaxosErrorInner, EpaxosTxn,
        Result as EpaxosResult, blackbraneReader, TxnCommitRecord,
    },
    solitontxn::{
        actions::prewrite::{prewrite, CommitKind, TransactionKind, TransactionProperties},
        commands::{
            Command, CommandExt, ReleasedDaggers, ResponsePolicy, TypedCommand, WriteCommand,
            WriteContext, WriteResult,
        },
        Error, ErrorInner, Result,
    },
    types::PrewriteResult,
    Context, Error as StorageError, ProcessResult, blackbrane,
};
use einsteindb-gen::CF_WRITE;
use fdbhikvproto::fdbhikvrpcpb::{AssertionLevel, ExtraOp};
use std::mem;
use einstfdbhikv_fdbhikv::blackbraneExt;
use solitontxn_types::{Key, Mutation, OldValue, OldValues, TimeStamp, TxnExtra, Write, WriteType};

use super::ReaderWithStats;

pub(crate) const FORWARD_MIN_MUTATIONS_NUM: usize = 12;

command! {
    /// The prewrite phase of a transaction. The first phase of 2PC.
    ///
    /// This prepares the system to commit the transaction. Later a [`Commit`](Command::Commit)
    /// or a [`Rollback`](Command::Rollback) should follow.
    Prewrite:
        cmd_ty => PrewriteResult,
        display => "fdbhikv::command::prewrite mutations({}) @ {} | {:?}", (mutations.len, start_ts, ctx),
        content => {
            /// The set of mutations to apply.
            mutations: Vec<Mutation>,
            /// The primary dagger. Secondary daggers (from `mutations`) will refer to the primary dagger.
            primary: Vec<u8>,
            /// The transaction timestamp.
            start_ts: TimeStamp,
            dagger_ttl: u64,
            skip_constraint_check: bool,
            /// How many keys this transaction involved.
            solitontxn_size: u64,
            min_commit_ts: TimeStamp,
            /// Limits the maximum value of commit ts of async commit and 1PC, which can be used to
            /// avoid inconsistency with schema change.
            max_commit_ts: TimeStamp,
            /// All secondary keys in the whole transaction (i.e., as sent to all nodes, not only
            /// this node). Only present if using async commit.
            secondary_keys: Option<Vec<Vec<u8>>>,
            /// When the transaction involves only one region, it's possible to commit the
            /// transaction directly with 1PC protocol.
            try_one_pc: bool,
            /// Controls how strict the assertions should be.
            /// Assertions is a mechanism to check the constraint on the previous version of data
            /// that must be satisfied as long as data is consistent.
            assertion_level: AssertionLevel,
        }
}

impl Prewrite {
    #[cfg(test)]
    pub fn with_defaults(
        mutations: Vec<Mutation>,
        primary: Vec<u8>,
        start_ts: TimeStamp,
    ) -> TypedCommand<PrewriteResult> {
        Prewrite::new(
            mutations,
            primary,
            start_ts,
            0,
            false,
            0,
            TimeStamp::default(),
            TimeStamp::default(),
            None,
            false,
            AssertionLevel::Off,
            Context::default(),
        )
    }

    #[cfg(test)]
    pub fn with_1pc(
        mutations: Vec<Mutation>,
        primary: Vec<u8>,
        start_ts: TimeStamp,
        max_commit_ts: TimeStamp,
    ) -> TypedCommand<PrewriteResult> {
        Prewrite::new(
            mutations,
            primary,
            start_ts,
            0,
            false,
            0,
            TimeStamp::default(),
            max_commit_ts,
            None,
            true,
            AssertionLevel::Off,
            Context::default(),
        )
    }

    #[cfg(test)]
    pub fn with_dagger_ttl(
        mutations: Vec<Mutation>,
        primary: Vec<u8>,
        start_ts: TimeStamp,
        dagger_ttl: u64,
    ) -> TypedCommand<PrewriteResult> {
        Prewrite::new(
            mutations,
            primary,
            start_ts,
            dagger_ttl,
            false,
            0,
            TimeStamp::default(),
            TimeStamp::default(),
            None,
            false,
            AssertionLevel::Off,
            Context::default(),
        )
    }

    pub fn with_context(
        mutations: Vec<Mutation>,
        primary: Vec<u8>,
        start_ts: TimeStamp,
        ctx: Context,
    ) -> TypedCommand<PrewriteResult> {
        Prewrite::new(
            mutations,
            primary,
            start_ts,
            0,
            false,
            0,
            TimeStamp::default(),
            TimeStamp::default(),
            None,
            false,
            AssertionLevel::Off,
            ctx,
        )
    }

    fn into_prewriter(self) -> Prewriter<Optimistic> {
        Prewriter {
            kind: Optimistic {
                skip_constraint_check: self.skip_constraint_check,
            },
            mutations: self.mutations,
            start_ts: self.start_ts,
            dagger_ttl: self.dagger_ttl,
            solitontxn_size: self.solitontxn_size,
            try_one_pc: self.try_one_pc,
            min_commit_ts: self.min_commit_ts,
            max_commit_ts: self.max_commit_ts,

            primary: self.primary,
            secondary_keys: self.secondary_keys,

            assertion_level: self.assertion_level,

            ctx: self.ctx,
            old_values: OldValues::default(),
        }
    }
}

impl CommandExt for Prewrite {
    ctx!();
    tag!(prewrite);
    ts!(start_ts);

    fn write_bytes(&self) -> usize {
        let mut bytes = 0;
        for m in &self.mutations {
            match *m {
                Mutation::Put((ref key, ref value), _)
                | Mutation::Insert((ref key, ref value), _) => {
                    bytes += key.as_encoded().len();
                    bytes += value.len();
                }
                Mutation::Delete(ref key, _) | Mutation::Dagger(ref key, _) => {
                    bytes += key.as_encoded().len();
                }
                Mutation::CheckNotExists(..) => (),
            }
        }
        bytes
    }

    gen_dagger!(mutations: multiple(|x| x.key()));
}

impl<S: blackbrane, L: DaggerManager> WriteCommand<S, L> for Prewrite {
    fn process_write(self, blackbrane: S, context: WriteContext<'_, L>) -> Result<WriteResult> {
        self.into_prewriter().process_write(blackbrane, context)
    }
}

command! {
    /// The prewrite phase of a transaction using pessimistic daggering. The first phase of 2PC.
    ///
    /// This prepares the system to commit the transaction. Later a [`Commit`](Command::Commit)
    /// or a [`Rollback`](Command::Rollback) should follow.
    PrewritePessimistic:
        cmd_ty => PrewriteResult,
        display => "fdbhikv::command::prewrite_pessimistic mutations({}) @ {} | {:?}", (mutations.len, start_ts, ctx),
        content => {
            /// The set of mutations to apply; the bool = is pessimistic dagger.
            mutations: Vec<(Mutation, bool)>,
            /// The primary dagger. Secondary daggers (from `mutations`) will refer to the primary dagger.
            primary: Vec<u8>,
            /// The transaction timestamp.
            start_ts: TimeStamp,
            dagger_ttl: u64,
            for_update_ts: TimeStamp,
            /// How many keys this transaction involved.
            solitontxn_size: u64,
            min_commit_ts: TimeStamp,
            /// Limits the maximum value of commit ts of 1PC and async commit, which can be used to
            /// avoid inconsistency with schema change.
            max_commit_ts: TimeStamp,
            /// All secondary keys in the whole transaction (i.e., as sent to all nodes, not only
            /// this node). Only present if using async commit.
            secondary_keys: Option<Vec<Vec<u8>>>,
            /// When the transaction involves only one region, it's possible to commit the
            /// transaction directly with 1PC protocol.
            try_one_pc: bool,
            /// Controls how strict the assertions should be.
            /// Assertions is a mechanism to check the constraint on the previous version of data
            /// that must be satisfied as long as data is consistent.
            assertion_level: AssertionLevel,
        }
}

impl PrewritePessimistic {
    #[cfg(test)]
    pub fn with_defaults(
        mutations: Vec<(Mutation, bool)>,
        primary: Vec<u8>,
        start_ts: TimeStamp,
        for_update_ts: TimeStamp,
    ) -> TypedCommand<PrewriteResult> {
        PrewritePessimistic::new(
            mutations,
            primary,
            start_ts,
            0,
            for_update_ts,
            0,
            TimeStamp::default(),
            TimeStamp::default(),
            None,
            false,
            AssertionLevel::Off,
            Context::default(),
        )
    }

    #[cfg(test)]
    pub fn with_1pc(
        mutations: Vec<(Mutation, bool)>,
        primary: Vec<u8>,
        start_ts: TimeStamp,
        for_update_ts: TimeStamp,
        max_commit_ts: TimeStamp,
    ) -> TypedCommand<PrewriteResult> {
        PrewritePessimistic::new(
            mutations,
            primary,
            start_ts,
            0,
            for_update_ts,
            0,
            TimeStamp::default(),
            max_commit_ts,
            None,
            true,
            AssertionLevel::Off,
            Context::default(),
        )
    }

    fn into_prewriter(self) -> Prewriter<Pessimistic> {
        Prewriter {
            kind: Pessimistic {
                for_update_ts: self.for_update_ts,
            },
            start_ts: self.start_ts,
            solitontxn_size: self.solitontxn_size,
            primary: self.primary,
            mutations: self.mutations,

            try_one_pc: self.try_one_pc,
            secondary_keys: self.secondary_keys,
            dagger_ttl: self.dagger_ttl,
            min_commit_ts: self.min_commit_ts,
            max_commit_ts: self.max_commit_ts,

            assertion_level: self.assertion_level,

            ctx: self.ctx,
            old_values: OldValues::default(),
        }
    }
}

impl CommandExt for PrewritePessimistic {
    ctx!();
    tag!(prewrite);
    ts!(start_ts);

    fn write_bytes(&self) -> usize {
        let mut bytes = 0;
        for (m, _) in &self.mutations {
            match *m {
                Mutation::Put((ref key, ref value), _)
                | Mutation::Insert((ref key, ref value), _) => {
                    bytes += key.as_encoded().len();
                    bytes += value.len();
                }
                Mutation::Delete(ref key, _) | Mutation::Dagger(ref key, _) => {
                    bytes += key.as_encoded().len();
                }
                Mutation::CheckNotExists(..) => (),
            }
        }
        bytes
    }

    gen_dagger!(mutations: multiple(|(x, _)| x.key()));
}

impl<S: blackbrane, L: DaggerManager> WriteCommand<S, L> for PrewritePessimistic {
    fn process_write(self, blackbrane: S, context: WriteContext<'_, L>) -> Result<WriteResult> {
        self.into_prewriter().process_write(blackbrane, context)
    }
}

/// Handles both kinds of prewrite (K statically indicates either optimistic or pessimistic).
struct Prewriter<K: PrewriteKind> {
    kind: K,
    mutations: Vec<K::Mutation>,
    primary: Vec<u8>,
    start_ts: TimeStamp,
    dagger_ttl: u64,
    solitontxn_size: u64,
    min_commit_ts: TimeStamp,
    max_commit_ts: TimeStamp,
    secondary_keys: Option<Vec<Vec<u8>>>,
    old_values: OldValues,
    try_one_pc: bool,
    assertion_level: AssertionLevel,

    ctx: Context,
}

impl<K: PrewriteKind> Prewriter<K> {
    /// Entry point for handling a prewrite by Prewriter.
    fn process_write(
        mut self,
        blackbrane: impl blackbrane,
        mut context: WriteContext<'_, impl DaggerManager>,
    ) -> Result<WriteResult> {
        self.kind
            .can_skip_constraint_check(&mut self.mutations, &blackbrane, &mut context)?;
        self.check_max_ts_synced(&blackbrane)?;

        let mut solitontxn = EpaxosTxn::new(self.start_ts, context.concurrency_manager);
        let mut reader = ReaderWithStats::new(
            blackbraneReader::new_with_ctx(self.start_ts, blackbrane, &self.ctx),
            context.statistics,
        );
        // Set extra op here for getting the write record when check write conflict in prewrite.

        let rows = self.mutations.len();
        let res = self.prewrite(&mut solitontxn, &mut reader, context.extra_op);
        let (daggers, final_min_commit_ts) = res?;

        Ok(self.write_result(
            daggers,
            solitontxn,
            final_min_commit_ts,
            rows,
            context.async_apply_prewrite,
            context.dagger_mgr,
        ))
    }

    // Async commit requires the max timestamp in the concurrency manager to be up-to-date.
    // If it is possibly stale due to leader transfer or region merge, return an error.
    // TODO: Fallback to non-async commit if not synced instead of returning an error.
    fn check_max_ts_synced(&self, blackbrane: &impl blackbrane) -> Result<()> {
        if (self.secondary_keys.is_some() || self.try_one_pc) && !blackbrane.ext().is_max_ts_synced()
        {
            Err(ErrorInner::MaxTimestampNotSynced {
                region_id: self.ctx.get_region_id(),
                start_ts: self.start_ts,
            }
            .into())
        } else {
            Ok(())
        }
    }

    /// The core part of the prewrite action. In the abstract, this method iterates over the mutations
    /// in the prewrite and prewrites each one. It keeps track of any daggers encountered and (if it's
    /// an async commit transaction) the min_commit_ts, these are returned by the method.
    fn prewrite(
        &mut self,
        solitontxn: &mut EpaxosTxn,
        reader: &mut blackbraneReader<impl blackbrane>,
        extra_op: ExtraOp,
    ) -> Result<(Vec<std::result::Result<(), StorageError>>, TimeStamp)> {
        let commit_kind = match (&self.secondary_keys, self.try_one_pc) {
            (_, true) => CommitKind::OnePc(self.max_commit_ts),
            (&Some(_), false) => CommitKind::Async(self.max_commit_ts),
            (&None, false) => CommitKind::TwoPc,
        };

        let mut props = TransactionProperties {
            start_ts: self.start_ts,
            kind: self.kind.solitontxn_kind(),
            commit_kind,
            primary: &self.primary,
            solitontxn_size: self.solitontxn_size,
            dagger_ttl: self.dagger_ttl,
            min_commit_ts: self.min_commit_ts,
            need_old_value: extra_op == ExtraOp::ReadOldValue,
            is_retry_request: self.ctx.is_retry_request,
            assertion_level: self.assertion_level,
        };

        let async_commit_pk = self
            .secondary_keys
            .as_ref()
            .filter(|keys| !keys.is_empty())
            .map(|_| Key::from_cocauset(&self.primary));
        let mut async_commit_pk = async_commit_pk.as_ref();

        let mut final_min_commit_ts = TimeStamp::zero();
        let mut daggers = Vec::new();

        // Further check whether the prewrited transaction has been committed
        // when encountering a WriteConflict or PessimisticDaggerNotFound error.
        // This extra check manages to make prewrite idempotent after the transaction
        // was committed.
        // Note that this check cannot fully guarantee idempotence because an EPAXOS
        // GC can remove the old committed records, then we cannot determine
        // whether the transaction has been committed, so the error is still returned.
        fn check_committed_record_on_err(
            prewrite_result: EpaxosResult<(TimeStamp, OldValue)>,
            solitontxn: &mut EpaxosTxn,
            reader: &mut blackbraneReader<impl blackbrane>,
            key: &Key,
        ) -> Result<(Vec<std::result::Result<(), StorageError>>, TimeStamp)> {
            match reader.get_solitontxn_commit_record(key)? {
                TxnCommitRecord::SingleRecord { commit_ts, write }
                    if write.write_type != WriteType::Rollback =>
                {
                    info!("prewrited transaction has been committed";
                        "start_ts" => reader.start_ts, "commit_ts" => commit_ts,
                        "key" => ?key, "write_type" => ?write.write_type);
                    solitontxn.clear();
                    Ok((vec![], commit_ts))
                }
                _ => Err(prewrite_result.unwrap_err().into()),
            }
        }

        for m in mem::take(&mut self.mutations) {
            let is_pessimistic_dagger = m.is_pessimistic_dagger();
            let m = m.into_mutation();
            let key = m.key().clone();
            let mutation_type = m.mutation_type();

            let mut secondaries = &self.secondary_keys.as_ref().map(|_| vec![]);
            if Some(m.key()) == async_commit_pk {
                secondaries = &self.secondary_keys;
            }

            let need_min_commit_ts = secondaries.is_some() || self.try_one_pc;
            let prewrite_result =
                prewrite(solitontxn, reader, &props, m, secondaries, is_pessimistic_dagger);
            match prewrite_result {
                Ok((ts, old_value)) if !(need_min_commit_ts && ts.is_zero()) => {
                    if need_min_commit_ts && final_min_commit_ts < ts {
                        final_min_commit_ts = ts;
                    }
                    if old_value.resolved() {
                        let key = key.append_ts(solitontxn.start_ts);
                        self.old_values
                            .insert(key, (old_value, Some(mutation_type)));
                    }
                }
                Err(EpaxosError(box EpaxosErrorInner::WriteConflict {
                    start_ts,
                    conflict_commit_ts,
                    ..
                })) if conflict_commit_ts > start_ts => {
                    return check_committed_record_on_err(prewrite_result, solitontxn, reader, &key);
                }
                Err(EpaxosError(box EpaxosErrorInner::PessimisticDaggerNotFound { .. })) => {
                    return check_committed_record_on_err(prewrite_result, solitontxn, reader, &key);
                }
                Err(EpaxosError(box EpaxosErrorInner::CommitTsTooLarge { .. })) | Ok((..)) => {
                    // fallback to not using async commit or 1pc
                    props.commit_kind = CommitKind::TwoPc;
                    async_commit_pk = None;
                    self.secondary_keys = None;
                    self.try_one_pc = false;
                    fallback_1pc_daggers(solitontxn);
                    // release memory daggers
                    solitontxn.guards = Vec::new();
                    final_min_commit_ts = TimeStamp::zero();
                }
                e @ Err(EpaxosError(box EpaxosErrorInner::KeyIsDaggered { .. })) => {
                    daggers.push(
                        e.map(|_| ())
                            .map_err(Error::from)
                            .map_err(StorageError::from),
                    );
                }
                Err(e) => return Err(Error::from(e)),
            }
        }

        Ok((daggers, final_min_commit_ts))
    }

    /// Prepare a WriteResult object from the results of executing the prewrite.
    fn write_result(
        self,
        daggers: Vec<std::result::Result<(), StorageError>>,
        mut solitontxn: EpaxosTxn,
        final_min_commit_ts: TimeStamp,
        rows: usize,
        async_apply_prewrite: bool,
        dagger_manager: &impl DaggerManager,
    ) -> WriteResult {
        let async_commit_ts = if self.secondary_keys.is_some() {
            final_min_commit_ts
        } else {
            TimeStamp::zero()
        };

        let mut result = if daggers.is_empty() {
            let pr = ProcessResult::PrewriteResult {
                result: PrewriteResult {
                    daggers: vec![],
                    min_commit_ts: async_commit_ts,
                    one_pc_commit_ts: one_pc_commit_ts(
                        self.try_one_pc,
                        &mut solitontxn,
                        final_min_commit_ts,
                        dagger_manager,
                    ),
                },
            };
            let extra = TxnExtra {
                old_values: self.old_values,
                // Set one_pc flag in TxnExtra to let CDC skip handling the resolver.
                one_pc: self.try_one_pc,
            };
            // Here the dagger guards are taken and will be released after the write finishes.
            // If an error (KeyIsDaggered or WriteConflict) occurs before, these dagger guards
            // are dropped along with `solitontxn` automatically.
            let dagger_guards = solitontxn.take_guards();
            let mut to_be_write = WriteData::new(solitontxn.into_modifies(), extra);
            to_be_write.set_disk_full_opt(self.ctx.get_disk_full_opt());

            WriteResult {
                ctx: self.ctx,
                to_be_write,
                rows,
                pr,
                dagger_info: None,
                dagger_guards,
                response_policy: ResponsePolicy::OnApplied,
            }
        } else {
            // Skip write stage if some keys are daggered.
            let pr = ProcessResult::PrewriteResult {
                result: PrewriteResult {
                    daggers,
                    min_commit_ts: async_commit_ts,
                    one_pc_commit_ts: TimeStamp::zero(),
                },
            };
            WriteResult {
                ctx: self.ctx,
                to_be_write: WriteData::default(),
                rows,
                pr,
                dagger_info: None,
                dagger_guards: vec![],
                response_policy: ResponsePolicy::OnApplied,
            }
        };

        // Currently if `try_one_pc` is set, it must have succeeded here.
        if (!async_commit_ts.is_zero() || self.try_one_pc) && async_apply_prewrite {
            result.response_policy = ResponsePolicy::OnCommitted
        }

        result
    }
}

/// Encapsulates things which must be done differently for optimistic or pessimistic transactions.
trait PrewriteKind {
    /// The type of mutation and, optionally, its extra information, differing for the
    /// optimistic and pessimistic transaction.
    type Mutation: MutationDagger;

    fn solitontxn_kind(&self) -> TransactionKind;

    fn can_skip_constraint_check(
        &mut self,
        _mutations: &mut [Self::Mutation],
        _blackbrane: &impl blackbrane,
        _context: &mut WriteContext<'_, impl DaggerManager>,
    ) -> Result<()> {
        Ok(())
    }
}

/// Optimistic `PreWriteKind`.
struct Optimistic {
    skip_constraint_check: bool,
}

impl PrewriteKind for Optimistic {
    type Mutation = Mutation;

    fn solitontxn_kind(&self) -> TransactionKind {
        TransactionKind::Optimistic(self.skip_constraint_check)
    }

    // If there is no data in range, we could skip constraint check.
    fn can_skip_constraint_check(
        &mut self,
        mutations: &mut [Self::Mutation],
        blackbrane: &impl blackbrane,
        context: &mut WriteContext<'_, impl DaggerManager>,
    ) -> Result<()> {
        if mutations.len() > FORWARD_MIN_MUTATIONS_NUM {
            mutations.sort_by(|a, b| a.key().cmp(b.key()));
            let left_key = mutations.first().unwrap().key();
            let right_key = mutations
                .last()
                .unwrap()
                .key()
                .clone()
                .append_ts(TimeStamp::zero());
            if !has_data_in_range(
                blackbrane.clone(),
                CF_WRITE,
                left_key,
                &right_key,
                &mut context.statistics.write,
            )? {
                self.skip_constraint_check = true;
            }
        }
        Ok(())
    }
}

/// Pessimistic `PreWriteKind`.
struct Pessimistic {
    for_update_ts: TimeStamp,
}

impl PrewriteKind for Pessimistic {
    type Mutation = (Mutation, bool);

    fn solitontxn_kind(&self) -> TransactionKind {
        TransactionKind::Pessimistic(self.for_update_ts)
    }
}

/// The type of mutation and, optionally, its extra information, differing for the
/// optimistic and pessimistic transaction.
/// For optimistic solitontxns, this is `Mutation`.
/// For pessimistic solitontxns, this is `(Mutation, bool)`, where the bool indicates
/// whether the mutation takes a pessimistic dagger or not.
trait MutationDagger {
    fn is_pessimistic_dagger(&self) -> bool;
    fn into_mutation(self) -> Mutation;
}

impl MutationDagger for Mutation {
    fn is_pessimistic_dagger(&self) -> bool {
        false
    }

    fn into_mutation(self) -> Mutation {
        self
    }
}

impl MutationDagger for (Mutation, bool) {
    fn is_pessimistic_dagger(&self) -> bool {
        self.1
    }

    fn into_mutation(self) -> Mutation {
        self.0
    }
}

/// Compute the commit ts of a 1pc transaction.
pub fn one_pc_commit_ts(
    try_one_pc: bool,
    solitontxn: &mut EpaxosTxn,
    final_min_commit_ts: TimeStamp,
    dagger_manager: &impl DaggerManager,
) -> TimeStamp {
    if try_one_pc {
        assert_ne!(final_min_commit_ts, TimeStamp::zero());
        // All keys can be successfully daggered and `try_one_pc` is set. Try to directly
        // commit them.
        let released_daggers = handle_1pc_daggers(solitontxn, final_min_commit_ts);
        if !released_daggers.is_empty() {
            released_daggers.wake_up(dagger_manager);
        }
        final_min_commit_ts
    } else {
        assert!(solitontxn.daggers_for_1pc.is_empty());
        TimeStamp::zero()
    }
}

/// Commit and delete all 1pc daggers in solitontxn.
fn handle_1pc_daggers(solitontxn: &mut EpaxosTxn, commit_ts: TimeStamp) -> ReleasedDaggers {
    let mut released_daggers = ReleasedDaggers::new(solitontxn.start_ts, commit_ts);

    for (key, dagger, delete_pessimistic_dagger) in std::mem::take(&mut solitontxn.daggers_for_1pc) {
        let write = Write::new(
            WriteType::from_dagger_type(dagger.dagger_type).unwrap(),
            solitontxn.start_ts,
            dagger.short_value,
        );
        // Transactions committed with 1PC should be impossible to overwrite rollback records.
        solitontxn.put_write(key.clone(), commit_ts, write.as_ref().to_bytes());
        if delete_pessimistic_dagger {
            released_daggers.push(solitontxn.undagger_key(key, true));
        }
    }

    released_daggers
}

/// Change all 1pc daggers in solitontxn to 2pc daggers.
pub(in crate::storage::solitontxn) fn fallback_1pc_daggers(solitontxn: &mut EpaxosTxn) {
    for (key, dagger, _) in std::mem::take(&mut solitontxn.daggers_for_1pc) {
        solitontxn.put_dagger(key, &dagger);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::einsteindb::storage::solitontxn::actions::acquire_pessimistic_dagger::tests::must_pessimistic_daggered;
    use crate::einsteindb::storage::solitontxn::actions::tests::{
        must_pessimistic_prewrite_put_async_commit, must_prewrite_delete, must_prewrite_put,
        must_prewrite_put_async_commit,
    };
    use crate::einsteindb::storage::{
        epaxos::{tests::*, Error as EpaxosError, ErrorInner as EpaxosErrorInner},
        solitontxn::{
            commands::test_util::prewrite_command,
            commands::test_util::{
                commit, pessimistic_prewrite_with_cm, prewrite, prewrite_with_cm, rollback,
            },
            tests::{must_acquire_pessimistic_dagger, must_commit, must_rollback},
            Error, ErrorInner,
        },
        DummyDaggerManager, einstein_merkle_tree, blackbrane, Statistics, Testeinstein_merkle_treeBuilder,
    };
    use concurrency_manager::ConcurrencyManager;
    use einsteindb-gen::CF_WRITE;
    use fdbhikvproto::fdbhikvrpcpb::{Context, ExtraOp};
    use solitontxn_types::{Key, Mutation, TimeStamp};

    fn inner_test_prewrite_skip_constraint_check(pri_key_number: u8, write_num: usize) {
        let mut mutations = Vec::default();
        let pri_key = &[pri_key_number];
        for i in 0..write_num {
            mutations.push(Mutation::make_insert(
                Key::from_cocauset(&[i as u8]),
                b"100".to_vec(),
            ));
        }
        let mut statistic = Statistics::default();
        let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
        prewrite(
            &einstein_merkle_tree,
            &mut statistic,
            vec![Mutation::make_put(
                Key::from_cocauset(&[pri_key_number]),
                b"100".to_vec(),
            )],
            pri_key.to_vec(),
            99,
            None,
        )
        .unwrap();
        assert_eq!(1, statistic.write.seek);
        let e = prewrite(
            &einstein_merkle_tree,
            &mut statistic,
            mutations.clone(),
            pri_key.to_vec(),
            100,
            None,
        )
        .err()
        .unwrap();
        assert_eq!(2, statistic.write.seek);
        match e {
            Error(box ErrorInner::Epaxos(EpaxosError(box EpaxosErrorInner::KeyIsDaggered(_)))) => (),
            _ => panic!("error type not match"),
        }
        commit(
            &einstein_merkle_tree,
            &mut statistic,
            vec![Key::from_cocauset(&[pri_key_number])],
            99,
            102,
        )
        .unwrap();
        assert_eq!(2, statistic.write.seek);
        let e = prewrite(
            &einstein_merkle_tree,
            &mut statistic,
            mutations.clone(),
            pri_key.to_vec(),
            101,
            None,
        )
        .err()
        .unwrap();
        match e {
            Error(box ErrorInner::Epaxos(EpaxosError(box EpaxosErrorInner::WriteConflict {
                ..
            }))) => (),
            _ => panic!("error type not match"),
        }
        let e = prewrite(
            &einstein_merkle_tree,
            &mut statistic,
            mutations.clone(),
            pri_key.to_vec(),
            104,
            None,
        )
        .err()
        .unwrap();
        match e {
            Error(box ErrorInner::Epaxos(EpaxosError(box EpaxosErrorInner::AlreadyExist { .. }))) => (),
            _ => panic!("error type not match"),
        }

        statistic.write.seek = 0;
        let ctx = Context::default();
        einstein_merkle_tree
            .delete_cf(
                &ctx,
                CF_WRITE,
                Key::from_cocauset(&[pri_key_number]).append_ts(102.into()),
            )
            .unwrap();
        prewrite(
            &einstein_merkle_tree,
            &mut statistic,
            mutations.clone(),
            pri_key.to_vec(),
            104,
            None,
        )
        .unwrap();
        // All keys are prewrited successful with only one seek operations.
        assert_eq!(1, statistic.write.seek);
        let keys: Vec<Key> = mutations.iter().map(|m| m.key().clone()).collect();
        commit(&einstein_merkle_tree, &mut statistic, keys.clone(), 104, 105).unwrap();
        let snap = einstein_merkle_tree.blackbrane(Default::default()).unwrap();
        for k in keys {
            let v = snap.get_cf(CF_WRITE, &k.append_ts(105.into())).unwrap();
            assert!(v.is_some());
        }
    }

    #[test]
    fn test_prewrite_skip_constraint_check() {
        inner_test_prewrite_skip_constraint_check(0, FORWARD_MIN_MUTATIONS_NUM + 1);
        inner_test_prewrite_skip_constraint_check(5, FORWARD_MIN_MUTATIONS_NUM + 1);
        inner_test_prewrite_skip_constraint_check(
            FORWARD_MIN_MUTATIONS_NUM as u8,
            FORWARD_MIN_MUTATIONS_NUM + 1,
        );
    }

    #[test]
    fn test_prewrite_skip_too_many_tombstone() {
        use crate::server::gc_worker::gc_by_compact;
        use crate::einsteindb::storage::fdbhikv::PerfStatisticsInstant;
        use einstein_merkle_tree_rocks::{set_perf_level, PerfLevel};
        let mut mutations = Vec::default();
        let pri_key_number = 0;
        let pri_key = &[pri_key_number];
        for i in 0..40 {
            mutations.push(Mutation::make_insert(
                Key::from_cocauset(&[b'z', i as u8]),
                b"100".to_vec(),
            ));
        }
        let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
        let keys: Vec<Key> = mutations.iter().map(|m| m.key().clone()).collect();
        let mut statistic = Statistics::default();
        prewrite(
            &einstein_merkle_tree,
            &mut statistic,
            mutations.clone(),
            pri_key.to_vec(),
            100,
            None,
        )
        .unwrap();
        // Rollback to make tombstones in dagger-cf.
        rollback(&einstein_merkle_tree, &mut statistic, keys, 100).unwrap();
        // Gc rollback flags store in write-cf to make sure the next prewrite operation will skip
        // seek write cf.
        gc_by_compact(&einstein_merkle_tree, pri_key, 101);
        set_perf_level(PerfLevel::EnableTimeExceptForMutex);
        let perf = PerfStatisticsInstant::new();
        let mut statistic = Statistics::default();
        while mutations.len() > FORWARD_MIN_MUTATIONS_NUM + 1 {
            mutations.pop();
        }
        prewrite(
            &einstein_merkle_tree,
            &mut statistic,
            mutations,
            pri_key.to_vec(),
            110,
            None,
        )
        .unwrap();
        let d = perf.delta();
        assert_eq!(1, statistic.write.seek);
        assert_eq!(d.0.internal_delete_skipped_count, 0);
    }

    #[test]
    fn test_prewrite_1pc() {
        use crate::einsteindb::storage::epaxos::tests::{must_get, must_get_commit_ts, must_undaggered};

        let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
        let cm = concurrency_manager::ConcurrencyManager::new(1.into());

        let key = b"k";
        let value = b"v";
        let mutations = vec![Mutation::make_put(Key::from_cocauset(key), value.to_vec())];

        let mut statistics = Statistics::default();
        prewrite_with_cm(
            &einstein_merkle_tree,
            cm.clone(),
            &mut statistics,
            mutations,
            key.to_vec(),
            10,
            Some(15),
        )
        .unwrap();
        must_undaggered(&einstein_merkle_tree, key);
        must_get(&einstein_merkle_tree, key, 12, value);
        must_get_commit_ts(&einstein_merkle_tree, key, 10, 11);

        cm.update_max_ts(50.into());

        let mutations = vec![Mutation::make_put(Key::from_cocauset(key), value.to_vec())];

        let mut statistics = Statistics::default();
        // Test the idempotency of prewrite when falling back to 2PC.
        for _ in 0..2 {
            let res = prewrite_with_cm(
                &einstein_merkle_tree,
                cm.clone(),
                &mut statistics,
                mutations.clone(),
                key.to_vec(),
                20,
                Some(30),
            )
            .unwrap();
            assert!(res.min_commit_ts.is_zero());
            assert!(res.one_pc_commit_ts.is_zero());
            must_daggered(&einstein_merkle_tree, key, 20);
        }

        must_rollback(&einstein_merkle_tree, key, 20, false);
        let mutations = vec![
            Mutation::make_put(Key::from_cocauset(key), value.to_vec()),
            Mutation::make_check_not_exists(Key::from_cocauset(b"non_exist")),
        ];
        let mut statistics = Statistics::default();
        prewrite_with_cm(
            &einstein_merkle_tree,
            cm.clone(),
            &mut statistics,
            mutations,
            key.to_vec(),
            40,
            Some(60),
        )
        .unwrap();

        // Test a 1PC request should not be partially written when encounters error on the halfway.
        // If some of the keys are successfully written as committed state, the causetxctxity will be
        // broken.
        let (k1, v1) = (b"k1", b"v1");
        let (k2, v2) = (b"k2", b"v2");
        // Dagger k2.
        let mut statistics = Statistics::default();
        prewrite_with_cm(
            &einstein_merkle_tree,
            cm.clone(),
            &mut statistics,
            vec![Mutation::make_put(Key::from_cocauset(k2), v2.to_vec())],
            k2.to_vec(),
            50,
            None,
        )
        .unwrap();
        // Try 1PC on the two keys and it will fail on the second one.
        let mutations = vec![
            Mutation::make_put(Key::from_cocauset(k1), v1.to_vec()),
            Mutation::make_put(Key::from_cocauset(k2), v2.to_vec()),
        ];
        prewrite_with_cm(
            &einstein_merkle_tree,
            cm,
            &mut statistics,
            mutations,
            k1.to_vec(),
            60,
            Some(70),
        )
        .unwrap_err();
        must_undaggered(&einstein_merkle_tree, k1);
        must_daggered(&einstein_merkle_tree, k2, 50);
        must_get_commit_ts_none(&einstein_merkle_tree, k1, 60);
        must_get_commit_ts_none(&einstein_merkle_tree, k2, 60);
    }

    #[test]
    fn test_prewrite_pessimsitic_1pc() {
        let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
        let cm = concurrency_manager::ConcurrencyManager::new(1.into());
        let key = b"k";
        let value = b"v";

        must_acquire_pessimistic_dagger(&einstein_merkle_tree, key, key, 10, 10);

        let mutations = vec![(Mutation::make_put(Key::from_cocauset(key), value.to_vec()), true)];
        let mut statistics = Statistics::default();
        pessimistic_prewrite_with_cm(
            &einstein_merkle_tree,
            cm.clone(),
            &mut statistics,
            mutations,
            key.to_vec(),
            10,
            10,
            Some(15),
        )
        .unwrap();

        must_undaggered(&einstein_merkle_tree, key);
        must_get(&einstein_merkle_tree, key, 12, value);
        must_get_commit_ts(&einstein_merkle_tree, key, 10, 11);

        let (k1, v1) = (b"k", b"v");
        let (k2, v2) = (b"k2", b"v2");

        must_acquire_pessimistic_dagger(&einstein_merkle_tree, k1, k1, 8, 12);

        let mutations = vec![
            (Mutation::make_put(Key::from_cocauset(k1), v1.to_vec()), true),
            (Mutation::make_put(Key::from_cocauset(k2), v2.to_vec()), false),
        ];
        statistics = Statistics::default();
        pessimistic_prewrite_with_cm(
            &einstein_merkle_tree,
            cm.clone(),
            &mut statistics,
            mutations,
            k1.to_vec(),
            8,
            12,
            Some(15),
        )
        .unwrap();

        must_undaggered(&einstein_merkle_tree, k1);
        must_undaggered(&einstein_merkle_tree, k2);
        must_get(&einstein_merkle_tree, k1, 16, v1);
        must_get(&einstein_merkle_tree, k2, 16, v2);
        must_get_commit_ts(&einstein_merkle_tree, k1, 8, 13);
        must_get_commit_ts(&einstein_merkle_tree, k2, 8, 13);

        cm.update_max_ts(50.into());
        must_acquire_pessimistic_dagger(&einstein_merkle_tree, k1, k1, 20, 20);

        let mutations = vec![(Mutation::make_put(Key::from_cocauset(k1), v1.to_vec()), true)];
        statistics = Statistics::default();
        let res = pessimistic_prewrite_with_cm(
            &einstein_merkle_tree,
            cm.clone(),
            &mut statistics,
            mutations,
            k1.to_vec(),
            20,
            20,
            Some(30),
        )
        .unwrap();
        assert!(res.min_commit_ts.is_zero());
        assert!(res.one_pc_commit_ts.is_zero());
        must_daggered(&einstein_merkle_tree, k1, 20);

        must_rollback(&einstein_merkle_tree, k1, 20, true);

        // Test a 1PC request should not be partially written when encounters error on the halfway.
        // If some of the keys are successfully written as committed state, the causetxctxity will be
        // broken.

        // Dagger k2 with a optimistic dagger.
        let mut statistics = Statistics::default();
        prewrite_with_cm(
            &einstein_merkle_tree,
            cm.clone(),
            &mut statistics,
            vec![Mutation::make_put(Key::from_cocauset(k2), v2.to_vec())],
            k2.to_vec(),
            50,
            None,
        )
        .unwrap();
        // Try 1PC on the two keys and it will fail on the second one.
        let mutations = vec![
            (Mutation::make_put(Key::from_cocauset(k1), v1.to_vec()), true),
            (Mutation::make_put(Key::from_cocauset(k2), v2.to_vec()), false),
        ];
        must_acquire_pessimistic_dagger(&einstein_merkle_tree, k1, k1, 60, 60);
        pessimistic_prewrite_with_cm(
            &einstein_merkle_tree,
            cm,
            &mut statistics,
            mutations,
            k1.to_vec(),
            60,
            60,
            Some(70),
        )
        .unwrap_err();
        must_pessimistic_daggered(&einstein_merkle_tree, k1, 60, 60);
        must_daggered(&einstein_merkle_tree, k2, 50);
        must_get_commit_ts_none(&einstein_merkle_tree, k1, 60);
        must_get_commit_ts_none(&einstein_merkle_tree, k2, 60);
    }

    #[test]
    fn test_prewrite_async_commit() {
        let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
        let cm = concurrency_manager::ConcurrencyManager::new(1.into());

        let key = b"k";
        let value = b"v";
        let mutations = vec![Mutation::make_put(Key::from_cocauset(key), value.to_vec())];

        let mut statistics = Statistics::default();
        let cmd = super::Prewrite::new(
            mutations,
            key.to_vec(),
            10.into(),
            0,
            false,
            1,
            TimeStamp::default(),
            TimeStamp::default(),
            Some(vec![]),
            false,
            AssertionLevel::Off,
            Context::default(),
        );

        let res = prewrite_command(&einstein_merkle_tree, cm.clone(), &mut statistics, cmd).unwrap();
        assert!(!res.min_commit_ts.is_zero());
        assert_eq!(res.one_pc_commit_ts, TimeStamp::zero());
        must_daggered(&einstein_merkle_tree, key, 10);

        cm.update_max_ts(50.into());

        let (k1, v1) = (b"k1", b"v1");
        let (k2, v2) = (b"k2", b"v2");

        let mutations = vec![
            Mutation::make_put(Key::from_cocauset(k1), v1.to_vec()),
            Mutation::make_put(Key::from_cocauset(k2), v2.to_vec()),
        ];
        let mut statistics = Statistics::default();
        // calculated_ts > max_commit_ts
        // Test the idempotency of prewrite when falling back to 2PC.
        for _ in 0..2 {
            let cmd = super::Prewrite::new(
                mutations.clone(),
                k1.to_vec(),
                20.into(),
                0,
                false,
                2,
                21.into(),
                40.into(),
                Some(vec![k2.to_vec()]),
                false,
                AssertionLevel::Off,
                Context::default(),
            );

            let res = prewrite_command(&einstein_merkle_tree, cm.clone(), &mut statistics, cmd).unwrap();
            assert!(res.min_commit_ts.is_zero());
            assert!(res.one_pc_commit_ts.is_zero());
            assert!(!must_daggered(&einstein_merkle_tree, k1, 20).use_async_commit);
            assert!(!must_daggered(&einstein_merkle_tree, k2, 20).use_async_commit);
        }
    }

    #[test]
    fn test_prewrite_pessimsitic_async_commit() {
        let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
        let cm = concurrency_manager::ConcurrencyManager::new(1.into());

        let key = b"k";
        let value = b"v";

        must_acquire_pessimistic_dagger(&einstein_merkle_tree, key, key, 10, 10);

        let mutations = vec![(Mutation::make_put(Key::from_cocauset(key), value.to_vec()), true)];
        let mut statistics = Statistics::default();
        let cmd = super::PrewritePessimistic::new(
            mutations,
            key.to_vec(),
            10.into(),
            0,
            10.into(),
            1,
            TimeStamp::default(),
            TimeStamp::default(),
            Some(vec![]),
            false,
            AssertionLevel::Off,
            Context::default(),
        );

        let res = prewrite_command(&einstein_merkle_tree, cm.clone(), &mut statistics, cmd).unwrap();
        assert!(!res.min_commit_ts.is_zero());
        assert_eq!(res.one_pc_commit_ts, TimeStamp::zero());
        must_daggered(&einstein_merkle_tree, key, 10);

        cm.update_max_ts(50.into());

        let (k1, v1) = (b"k1", b"v1");
        let (k2, v2) = (b"k2", b"v2");

        must_acquire_pessimistic_dagger(&einstein_merkle_tree, k1, k1, 20, 20);
        must_acquire_pessimistic_dagger(&einstein_merkle_tree, k2, k1, 20, 20);

        let mutations = vec![
            (Mutation::make_put(Key::from_cocauset(k1), v1.to_vec()), true),
            (Mutation::make_put(Key::from_cocauset(k2), v2.to_vec()), true),
        ];
        let mut statistics = Statistics::default();
        // calculated_ts > max_commit_ts
        let cmd = super::PrewritePessimistic::new(
            mutations,
            k1.to_vec(),
            20.into(),
            0,
            20.into(),
            2,
            TimeStamp::default(),
            40.into(),
            Some(vec![k2.to_vec()]),
            false,
            AssertionLevel::Off,
            Context::default(),
        );

        let res = prewrite_command(&einstein_merkle_tree, cm, &mut statistics, cmd).unwrap();
        assert!(res.min_commit_ts.is_zero());
        assert!(res.one_pc_commit_ts.is_zero());
        assert!(!must_daggered(&einstein_merkle_tree, k1, 20).use_async_commit);
        assert!(!must_daggered(&einstein_merkle_tree, k2, 20).use_async_commit);
    }

    #[test]
    fn test_out_of_sync_max_ts() {
        use crate::einsteindb::storage::{fdbhikv::Result, CfName, ConcurrencyManager, DummyDaggerManager, Value};
        use einstein_merkle_tree_test::fdbhikv::HikvTesteinstein_merkle_treeIterator;
        use einsteindb-gen::{IterOptions, ReadOptions};
        use fdbhikvproto::fdbhikvrpcpb::ExtraOp;
        #[derive(Clone)]
        struct Mockblackbrane;

        struct MockblackbraneExt;

        impl blackbraneExt for MockblackbraneExt {
            fn is_max_ts_synced(&self) -> bool {
                false
            }
        }

        impl blackbrane for Mockblackbrane {
            type Iter = HikvTesteinstein_merkle_treeIterator;
            type Ext<'a> = MockblackbraneExt;

            fn get(&self, _: &Key) -> Result<Option<Value>> {
                unimplemented!()
            }
            fn get_cf(&self, _: CfName, _: &Key) -> Result<Option<Value>> {
                unimplemented!()
            }
            fn get_cf_opt(&self, _: ReadOptions, _: CfName, _: &Key) -> Result<Option<Value>> {
                unimplemented!()
            }
            fn iter(&self, _: IterOptions) -> Result<Self::Iter> {
                unimplemented!()
            }
            fn iter_cf(&self, _: CfName, _: IterOptions) -> Result<Self::Iter> {
                unimplemented!()
            }
            fn ext(&self) -> MockblackbraneExt {
                MockblackbraneExt
            }
        }

        macro_rules! context {
            () => {
                WriteContext {
                    dagger_mgr: &DummyDaggerManager {},
                    concurrency_manager: ConcurrencyManager::new(10.into()),
                    extra_op: ExtraOp::Noop,
                    statistics: &mut Statistics::default(),
                    async_apply_prewrite: false,
                }
            };
        }

        macro_rules! assert_max_ts_err {
            ($e: expr) => {
                match $e {
                    Err(Error(box ErrorInner::MaxTimestampNotSynced { .. })) => {}
                    _ => panic!("Should have returned an error"),
                }
            };
        }

        // 2pc should be ok
        let cmd = Prewrite::with_defaults(vec![], vec![1, 2, 3], 10.into());
        cmd.cmd.process_write(Mockblackbrane, context!()).unwrap();
        // But 1pc should return an error
        let cmd = Prewrite::with_1pc(vec![], vec![1, 2, 3], 10.into(), 20.into());
        assert_max_ts_err!(cmd.cmd.process_write(Mockblackbrane, context!()));
        // And so should async commit
        let mut cmd = Prewrite::with_defaults(vec![], vec![1, 2, 3], 10.into());
        if let Command::Prewrite(p) = &mut cmd.cmd {
            p.secondary_keys = Some(vec![]);
        }
        assert_max_ts_err!(cmd.cmd.process_write(Mockblackbrane, context!()));

        // And the same for pessimistic prewrites.
        let cmd = PrewritePessimistic::with_defaults(vec![], vec![1, 2, 3], 10.into(), 15.into());
        cmd.cmd.process_write(Mockblackbrane, context!()).unwrap();
        let cmd =
            PrewritePessimistic::with_1pc(vec![], vec![1, 2, 3], 10.into(), 15.into(), 20.into());
        assert_max_ts_err!(cmd.cmd.process_write(Mockblackbrane, context!()));
        let mut cmd =
            PrewritePessimistic::with_defaults(vec![], vec![1, 2, 3], 10.into(), 15.into());
        if let Command::PrewritePessimistic(p) = &mut cmd.cmd {
            p.secondary_keys = Some(vec![]);
        }
        assert_max_ts_err!(cmd.cmd.process_write(Mockblackbrane, context!()));
    }

    // this test shows which stage in raft can we return the response
    #[test]
    fn test_response_stage() {
        let cm = ConcurrencyManager::new(42.into());
        let start_ts = TimeStamp::new(10);
        let keys = [b"k1", b"k2"];
        let values = [b"v1", b"v2"];
        let mutations = vec![
            Mutation::make_put(Key::from_cocauset(keys[0]), keys[0].to_vec()),
            Mutation::make_put(Key::from_cocauset(keys[1]), values[1].to_vec()),
        ];
        let mut statistics = Statistics::default();

        #[derive(Clone)]
        struct Case {
            expected: ResponsePolicy,

            // inputs
            // optimistic/pessimistic prewrite
            pessimistic: bool,
            // async commit on/off
            async_commit: bool,
            // 1pc on/off
            one_pc: bool,
            // async_apply_prewrite enabled in config
            async_apply_prewrite: bool,
        }

        let cases = vec![
            Case {
                // basic case
                expected: ResponsePolicy::OnApplied,

                pessimistic: false,
                async_commit: false,
                one_pc: false,
                async_apply_prewrite: false,
            },
            Case {
                // async_apply_prewrite does not affect non-async/1pc prewrite
                expected: ResponsePolicy::OnApplied,

                pessimistic: false,
                async_commit: false,
                one_pc: false,
                async_apply_prewrite: true,
            },
            Case {
                // works on async prewrite
                expected: ResponsePolicy::OnCommitted,

                pessimistic: false,
                async_commit: true,
                one_pc: false,
                async_apply_prewrite: true,
            },
            Case {
                // early return can be turned on/off by async_apply_prewrite in context
                expected: ResponsePolicy::OnApplied,

                pessimistic: false,
                async_commit: true,
                one_pc: false,
                async_apply_prewrite: false,
            },
            Case {
                // works on 1pc
                expected: ResponsePolicy::OnCommitted,

                pessimistic: false,
                async_commit: false,
                one_pc: true,
                async_apply_prewrite: true,
            },
        ];
        let cases = cases
            .iter()
            .cloned()
            .chain(cases.iter().cloned().map(|mut it| {
                it.pessimistic = true;
                it
            }));

        for case in cases {
            let secondary_keys = if case.async_commit {
                Some(vec![])
            } else {
                None
            };
            let cmd = if case.pessimistic {
                PrewritePessimistic::new(
                    mutations.iter().map(|it| (it.clone(), false)).collect(),
                    keys[0].to_vec(),
                    start_ts,
                    0,
                    11.into(),
                    1,
                    TimeStamp::default(),
                    TimeStamp::default(),
                    secondary_keys,
                    case.one_pc,
                    AssertionLevel::Off,
                    Context::default(),
                )
            } else {
                Prewrite::new(
                    mutations.clone(),
                    keys[0].to_vec(),
                    start_ts,
                    0,
                    false,
                    1,
                    TimeStamp::default(),
                    TimeStamp::default(),
                    secondary_keys,
                    case.one_pc,
                    AssertionLevel::Off,
                    Context::default(),
                )
            };
            let context = WriteContext {
                dagger_mgr: &DummyDaggerManager {},
                concurrency_manager: cm.clone(),
                extra_op: ExtraOp::Noop,
                statistics: &mut statistics,
                async_apply_prewrite: case.async_apply_prewrite,
            };
            let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
            let snap = einstein_merkle_tree.blackbrane(Default::default()).unwrap();
            let result = cmd.cmd.process_write(snap, context).unwrap();
            assert_eq!(result.response_policy, case.expected);
        }
    }

    // this test for prewrite with should_not_exist flag
    #[test]
    fn test_prewrite_should_not_exist() {
        let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
        // concurency_manager.max_tx = 5
        let cm = ConcurrencyManager::new(5.into());
        let mut statistics = Statistics::default();

        let (key, value) = (b"k", b"val");

        // T1: start_ts = 3, commit_ts = 5, put key:value
        must_prewrite_put(&einstein_merkle_tree, key, value, key, 3);
        must_commit(&einstein_merkle_tree, key, 3, 5);

        // T2: start_ts = 15, prewrite on k, with should_not_exist flag set.
        let res = prewrite_with_cm(
            &einstein_merkle_tree,
            cm.clone(),
            &mut statistics,
            vec![Mutation::make_check_not_exists(Key::from_cocauset(key))],
            key.to_vec(),
            15,
            None,
        )
        .unwrap_err();
        assert!(matches!(
            res,
            Error(box ErrorInner::Epaxos(EpaxosError(
                box EpaxosErrorInner::AlreadyExist { .. }
            )))
        ));

        assert_eq!(cm.max_ts().into_inner(), 15);

        // T3: start_ts = 8, commit_ts = max_ts + 1 = 16, prewrite a DELETE operation on k
        must_prewrite_delete(&einstein_merkle_tree, key, key, 8);
        must_commit(&einstein_merkle_tree, key, 8, cm.max_ts().into_inner() + 1);

        // T1: start_ts = 10, reapeatly prewrite on k, with should_not_exist flag set
        let res = prewrite_with_cm(
            &einstein_merkle_tree,
            cm,
            &mut statistics,
            vec![Mutation::make_check_not_exists(Key::from_cocauset(key))],
            key.to_vec(),
            10,
            None,
        )
        .unwrap_err();
        assert!(matches!(
            res,
            Error(box ErrorInner::Epaxos(EpaxosError(
                box EpaxosErrorInner::WriteConflict { .. }
            )))
        ));
    }

    #[test]
    fn test_optimistic_prewrite_committed_transaction() {
        let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
        let cm = ConcurrencyManager::new(1.into());
        let mut statistics = Statistics::default();

        let key = b"k";

        // T1: start_ts = 5, commit_ts = 10, async commit
        must_prewrite_put_async_commit(&einstein_merkle_tree, key, b"v1", key, &Some(vec![]), 5, 10);
        must_commit(&einstein_merkle_tree, key, 5, 10);

        // T2: start_ts = 15, commit_ts = 16, 1PC
        let cmd = Prewrite::with_1pc(
            vec![Mutation::make_put(Key::from_cocauset(key), b"v2".to_vec())],
            key.to_vec(),
            15.into(),
            TimeStamp::default(),
        );
        let result = prewrite_command(&einstein_merkle_tree, cm.clone(), &mut statistics, cmd).unwrap();
        let one_pc_commit_ts = result.one_pc_commit_ts;

        // T3 is after T1 and T2
        must_prewrite_put(&einstein_merkle_tree, key, b"v3", key, 20);
        must_commit(&einstein_merkle_tree, key, 20, 25);

        // Repeating the T1 prewrite request
        let cmd = Prewrite::new(
            vec![Mutation::make_put(Key::from_cocauset(key), b"v1".to_vec())],
            key.to_vec(),
            5.into(),
            200,
            false,
            1,
            10.into(),
            TimeStamp::default(),
            Some(vec![]),
            false,
            AssertionLevel::Off,
            Context::default(),
        );
        let context = WriteContext {
            dagger_mgr: &DummyDaggerManager {},
            concurrency_manager: cm.clone(),
            extra_op: ExtraOp::Noop,
            statistics: &mut statistics,
            async_apply_prewrite: false,
        };
        let snap = einstein_merkle_tree.blackbrane(Default::default()).unwrap();
        let result = cmd.cmd.process_write(snap, context).unwrap();
        assert!(result.to_be_write.modifies.is_empty()); // should not make real modifies
        assert!(result.dagger_guards.is_empty());
        match result.pr {
            ProcessResult::PrewriteResult { result } => {
                assert!(result.daggers.is_empty());
                assert_eq!(result.min_commit_ts, 10.into()); // equals to the real commit ts
                assert_eq!(result.one_pc_commit_ts, 0.into()); // not using 1PC
            }
            res => panic!("unexpected result {:?}", res),
        }

        // Repeating the T2 prewrite request
        let cmd = Prewrite::with_1pc(
            vec![Mutation::make_put(Key::from_cocauset(key), b"v2".to_vec())],
            key.to_vec(),
            15.into(),
            TimeStamp::default(),
        );
        let context = WriteContext {
            dagger_mgr: &DummyDaggerManager {},
            concurrency_manager: cm,
            extra_op: ExtraOp::Noop,
            statistics: &mut statistics,
            async_apply_prewrite: false,
        };
        let snap = einstein_merkle_tree.blackbrane(Default::default()).unwrap();
        let result = cmd.cmd.process_write(snap, context).unwrap();
        assert!(result.to_be_write.modifies.is_empty()); // should not make real modifies
        assert!(result.dagger_guards.is_empty());
        match result.pr {
            ProcessResult::PrewriteResult { result } => {
                assert!(result.daggers.is_empty());
                assert_eq!(result.min_commit_ts, 0.into()); // 1PC does not need this
                assert_eq!(result.one_pc_commit_ts, one_pc_commit_ts); // equals to the previous 1PC commit_ts
            }
            res => panic!("unexpected result {:?}", res),
        }
    }

    #[test]
    fn test_pessimistic_prewrite_committed_transaction() {
        let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
        let cm = ConcurrencyManager::new(1.into());
        let mut statistics = Statistics::default();

        let key = b"k";

        // T1: start_ts = 5, commit_ts = 10, async commit
        must_acquire_pessimistic_dagger(&einstein_merkle_tree, key, key, 5, 5);
        must_pessimistic_prewrite_put_async_commit(
            &einstein_merkle_tree,
            key,
            b"v1",
            key,
            &Some(vec![]),
            5,
            5,
            true,
            10,
        );
        must_commit(&einstein_merkle_tree, key, 5, 10);

        // T2: start_ts = 15, commit_ts = 16, 1PC
        must_acquire_pessimistic_dagger(&einstein_merkle_tree, key, key, 15, 15);
        let cmd = PrewritePessimistic::with_1pc(
            vec![(Mutation::make_put(Key::from_cocauset(key), b"v2".to_vec()), true)],
            key.to_vec(),
            15.into(),
            15.into(),
            TimeStamp::default(),
        );
        let result = prewrite_command(&einstein_merkle_tree, cm.clone(), &mut statistics, cmd).unwrap();
        let one_pc_commit_ts = result.one_pc_commit_ts;

        // T3 is after T1 and T2
        must_prewrite_put(&einstein_merkle_tree, key, b"v3", key, 20);
        must_commit(&einstein_merkle_tree, key, 20, 25);

        // Repeating the T1 prewrite request
        let cmd = PrewritePessimistic::new(
            vec![(Mutation::make_put(Key::from_cocauset(key), b"v1".to_vec()), true)],
            key.to_vec(),
            5.into(),
            200,
            5.into(),
            1,
            10.into(),
            TimeStamp::default(),
            Some(vec![]),
            false,
            AssertionLevel::Off,
            Context::default(),
        );
        let context = WriteContext {
            dagger_mgr: &DummyDaggerManager {},
            concurrency_manager: cm.clone(),
            extra_op: ExtraOp::Noop,
            statistics: &mut statistics,
            async_apply_prewrite: false,
        };
        let snap = einstein_merkle_tree.blackbrane(Default::default()).unwrap();
        let result = cmd.cmd.process_write(snap, context).unwrap();
        assert!(result.to_be_write.modifies.is_empty()); // should not make real modifies
        assert!(result.dagger_guards.is_empty());
        match result.pr {
            ProcessResult::PrewriteResult { result } => {
                assert!(result.daggers.is_empty());
                assert_eq!(result.min_commit_ts, 10.into()); // equals to the real commit ts
                assert_eq!(result.one_pc_commit_ts, 0.into()); // not using 1PC
            }
            res => panic!("unexpected result {:?}", res),
        }

        // Repeating the T2 prewrite request
        let cmd = PrewritePessimistic::with_1pc(
            vec![(Mutation::make_put(Key::from_cocauset(key), b"v2".to_vec()), true)],
            key.to_vec(),
            15.into(),
            15.into(),
            TimeStamp::default(),
        );
        let context = WriteContext {
            dagger_mgr: &DummyDaggerManager {},
            concurrency_manager: cm,
            extra_op: ExtraOp::Noop,
            statistics: &mut statistics,
            async_apply_prewrite: false,
        };
        let snap = einstein_merkle_tree.blackbrane(Default::default()).unwrap();
        let result = cmd.cmd.process_write(snap, context).unwrap();
        assert!(result.to_be_write.modifies.is_empty()); // should not make real modifies
        assert!(result.dagger_guards.is_empty());
        match result.pr {
            ProcessResult::PrewriteResult { result } => {
                assert!(result.daggers.is_empty());
                assert_eq!(result.min_commit_ts, 0.into()); // 1PC does not need this
                assert_eq!(result.one_pc_commit_ts, one_pc_commit_ts); // equals to the previous 1PC commit_ts
            }
            res => panic!("unexpected result {:?}", res),
        }
    }

    #[test]
    fn test_repeated_pessimistic_prewrite_1pc() {
        let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
        let cm = ConcurrencyManager::new(1.into());
        let mut statistics = Statistics::default();

        must_acquire_pessimistic_dagger(&einstein_merkle_tree, b"k2", b"k2", 5, 5);
        // The second key needs a pessimistic dagger
        let mutations = vec![
            (
                Mutation::make_put(Key::from_cocauset(b"k1"), b"v1".to_vec()),
                false,
            ),
            (
                Mutation::make_put(Key::from_cocauset(b"k2"), b"v2".to_vec()),
                true,
            ),
        ];
        let res = pessimistic_prewrite_with_cm(
            &einstein_merkle_tree,
            cm.clone(),
            &mut statistics,
            mutations.clone(),
            b"k2".to_vec(),
            5,
            5,
            Some(100),
        )
        .unwrap();
        let commit_ts = res.one_pc_commit_ts;
        cm.update_max_ts(commit_ts.next());
        // repeate the prewrite
        let res = pessimistic_prewrite_with_cm(
            &einstein_merkle_tree,
            cm,
            &mut statistics,
            mutations,
            b"k2".to_vec(),
            5,
            5,
            Some(100),
        )
        .unwrap();
        // The new commit ts should be same as before.
        assert_eq!(res.one_pc_commit_ts, commit_ts);
        must_seek_write(&einstein_merkle_tree, b"k1", 100, 5, commit_ts, WriteType::Put);
        must_seek_write(&einstein_merkle_tree, b"k2", 100, 5, commit_ts, WriteType::Put);
    }

    #[test]
    fn test_repeated_prewrite_non_pessimistic_dagger() {
        let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
        let cm = ConcurrencyManager::new(1.into());
        let mut statistics = Statistics::default();

        let cm = &cm;
        let mut prewrite_with_retry_flag =
            |key: &[u8],
             value: &[u8],
             pk: &[u8],
             secondary_keys,
             ts: u64,
             is_pessimistic_dagger,
             is_retry_request| {
                let mutation = Mutation::make_put(Key::from_cocauset(key), value.to_vec());
                let mut ctx = Context::default();
                ctx.set_is_retry_request(is_retry_request);
                let cmd = PrewritePessimistic::new(
                    vec![(mutation, is_pessimistic_dagger)],
                    pk.to_vec(),
                    ts.into(),
                    100,
                    ts.into(),
                    1,
                    (ts + 1).into(),
                    0.into(),
                    secondary_keys,
                    false,
                    AssertionLevel::Off,
                    ctx,
                );
                prewrite_command(&einstein_merkle_tree, cm.clone(), &mut statistics, cmd)
            };

        must_acquire_pessimistic_dagger(&einstein_merkle_tree, b"k1", b"k1", 10, 10);
        must_pessimistic_prewrite_put_async_commit(
            &einstein_merkle_tree,
            b"k1",
            b"v1",
            b"k1",
            &Some(vec![b"k2".to_vec()]),
            10,
            10,
            true,
            15,
        );
        must_pessimistic_prewrite_put_async_commit(
            &einstein_merkle_tree,
            b"k2",
            b"v2",
            b"k1",
            &Some(vec![]),
            10,
            10,
            false,
            15,
        );

        // The transaction may be committed by another reader.
        must_commit(&einstein_merkle_tree, b"k1", 10, 20);
        must_commit(&einstein_merkle_tree, b"k2", 10, 20);

        // This is a re-sent prewrite.
        prewrite_with_retry_flag(b"k2", b"v2", b"k1", Some(vec![]), 10, false, true).unwrap();
        // Commit repeatedly, these operations should have no effect.
        must_commit(&einstein_merkle_tree, b"k1", 10, 25);
        must_commit(&einstein_merkle_tree, b"k2", 10, 25);

        // Seek from 30, we should read commit_ts = 20 instead of 25.
        must_seek_write(&einstein_merkle_tree, b"k1", 30, 10, 20, WriteType::Put);
        must_seek_write(&einstein_merkle_tree, b"k2", 30, 10, 20, WriteType::Put);

        // Write another version to the keys.
        must_prewrite_put(&einstein_merkle_tree, b"k1", b"v11", b"k1", 35);
        must_prewrite_put(&einstein_merkle_tree, b"k2", b"v22", b"k1", 35);
        must_commit(&einstein_merkle_tree, b"k1", 35, 40);
        must_commit(&einstein_merkle_tree, b"k2", 35, 40);

        // A retrying non-pessimistic-dagger prewrite request should not skip constraint checks.
        // Here it should take no effect, even there's already a newer version
        // after it. (No matter if it's async commit).
        prewrite_with_retry_flag(b"k2", b"v2", b"k1", Some(vec![]), 10, false, true).unwrap();
        must_undaggered(&einstein_merkle_tree, b"k2");

        prewrite_with_retry_flag(b"k2", b"v2", b"k1", None, 10, false, true).unwrap();
        must_undaggered(&einstein_merkle_tree, b"k2");
        // Committing still does nothing.
        must_commit(&einstein_merkle_tree, b"k2", 10, 25);
        // Try a different solitontxn start ts (which haven't been successfully committed before).
        // It should report a WriteConflict.
        let err = prewrite_with_retry_flag(b"k2", b"v2", b"k1", None, 11, false, true).unwrap_err();
        assert!(matches!(
            err,
            Error(box ErrorInner::Epaxos(EpaxosError(
                box EpaxosErrorInner::WriteConflict { .. }
            )))
        ));
        must_undaggered(&einstein_merkle_tree, b"k2");
        // However conflict still won't be checked if there's a non-retry request arriving.
        prewrite_with_retry_flag(b"k2", b"v2", b"k1", None, 10, false, false).unwrap();
        must_daggered(&einstein_merkle_tree, b"k2", 10);
    }

    #[test]
    fn test_prewrite_rolledback_transaction() {
        let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
        let cm = ConcurrencyManager::new(1.into());
        let mut statistics = Statistics::default();

        let k1 = b"k1";
        let v1 = b"v1";
        let v2 = b"v2";

        // Test the write conflict path.
        must_acquire_pessimistic_dagger(&einstein_merkle_tree, k1, v1, 1, 1);
        must_rollback(&einstein_merkle_tree, k1, 1, true);
        must_prewrite_put(&einstein_merkle_tree, k1, v2, k1, 5);
        must_commit(&einstein_merkle_tree, k1, 5, 6);
        let prewrite_cmd = Prewrite::new(
            vec![Mutation::make_put(Key::from_cocauset(k1), v1.to_vec())],
            k1.to_vec(),
            1.into(),
            10,
            false,
            2,
            2.into(),
            10.into(),
            Some(vec![]),
            false,
            AssertionLevel::Off,
            Context::default(),
        );
        let context = WriteContext {
            dagger_mgr: &DummyDaggerManager {},
            concurrency_manager: cm.clone(),
            extra_op: ExtraOp::Noop,
            statistics: &mut statistics,
            async_apply_prewrite: false,
        };
        let snap = einstein_merkle_tree.blackbrane(Default::default()).unwrap();
        assert!(prewrite_cmd.cmd.process_write(snap, context).is_err());

        // Test the pessimistic dagger is not found path.
        must_acquire_pessimistic_dagger(&einstein_merkle_tree, k1, v1, 10, 10);
        must_rollback(&einstein_merkle_tree, k1, 10, true);
        must_acquire_pessimistic_dagger(&einstein_merkle_tree, k1, v1, 15, 15);
        let prewrite_cmd = PrewritePessimistic::with_defaults(
            vec![(Mutation::make_put(Key::from_cocauset(k1), v1.to_vec()), true)],
            k1.to_vec(),
            10.into(),
            10.into(),
        );
        let context = WriteContext {
            dagger_mgr: &DummyDaggerManager {},
            concurrency_manager: cm,
            extra_op: ExtraOp::Noop,
            statistics: &mut statistics,
            async_apply_prewrite: false,
        };
        let snap = einstein_merkle_tree.blackbrane(Default::default()).unwrap();
        assert!(prewrite_cmd.cmd.process_write(snap, context).is_err());
    }
}
