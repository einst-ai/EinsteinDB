// Copyright 2020 EinsteinDB Project Authors. Licensed under Apache-2.0.

// #[PerformanceCriticallocal_path]
use crate::einsteindb::storage::fdbhikv::WriteData;
use crate::einsteindb::storage::dagger_manager::DaggerManager;
use crate::einsteindb::storage::epaxos::{EpaxosTxn, blackbraneReader};
use crate::einsteindb::storage::solitontxn::commands::{
    Command, CommandExt, ReaderWithStats, ReleasedDaggers, ResponsePolicy, TypedCommand,
    WriteCommand, WriteContext, WriteResult,
};
use crate::einsteindb::storage::solitontxn::{cleanup, Result};
use crate::einsteindb::storage::{ProcessResult, blackbrane};
use solitontxn_types::{Key, TimeStamp};

command! {
    /// Rollback from the transaction that was started at `start_ts`.
    ///
    /// This should be following a [`Prewrite`](Command::Prewrite) on the given key.
    Rollback:
        cmd_ty => (),
        display => "fdbhikv::command::rollback keys({}) @ {} | {:?}", (keys.len, start_ts, ctx),
        content => {
            keys: Vec<Key>,
            /// The transaction timestamp.
            start_ts: TimeStamp,
        }
}

impl CommandExt for Rollback {
    ctx!();
    tag!(rollback);
    ts!(start_ts);
    write_bytes!(keys: multiple);
    gen_dagger!(keys: multiple);
}

impl<S: blackbrane, L: DaggerManager> WriteCommand<S, L> for Rollback {
    fn process_write(self, blackbrane: S, context: WriteContext<'_, L>) -> Result<WriteResult> {
        let mut solitontxn = EpaxosTxn::new(self.start_ts, context.concurrency_manager);
        let mut reader = ReaderWithStats::new(
            blackbraneReader::new_with_ctx(self.start_ts, blackbrane, &self.ctx),
            context.statistics,
        );

        let rows = self.keys.len();
        let mut released_daggers = ReleasedDaggers::new(self.start_ts, TimeStamp::zero());
        for k in self.keys {
            // Rollback is called only if the transaction is known to fail. Under the circumstances,
            // the rollback record needn't be protected.
            let released_dagger = cleanup(&mut solitontxn, &mut reader, k, TimeStamp::zero(), false)?;
            released_daggers.push(released_dagger);
        }
        released_daggers.wake_up(context.dagger_mgr);

        let mut write_data = WriteData::from_modifies(solitontxn.into_modifies());
        write_data.set_allowed_on_disk_almost_full();
        Ok(WriteResult {
            ctx: self.ctx,
            to_be_write: write_data,
            rows,
            pr: ProcessResult::Res,
            dagger_info: None,
            dagger_guards: vec![],
            response_policy: ResponsePolicy::OnApplied,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::einsteindb::storage::solitontxn::tests::*;
    use crate::einsteindb::storage::Testeinstein_merkle_treeBuilder;

    #[test]
    fn rollback_dagger_with_existing_rollback() {
        let einstein_merkle_tree = Testeinstein_merkle_treeBuilder::new().build().unwrap();
        let (k1, k2) = (b"k1", b"k2");
        let v = b"v";

        must_acquire_pessimistic_dagger(&einstein_merkle_tree, k1, k1, 10, 10);
        must_rollback(&einstein_merkle_tree, k1, 10, false);
        must_rollback(&einstein_merkle_tree, k2, 10, false);

        must_pessimistic_prewrite_put(&einstein_merkle_tree, k2, v, k1, 10, 10, false);
        must_rollback(&einstein_merkle_tree, k2, 10, false);
    }
}