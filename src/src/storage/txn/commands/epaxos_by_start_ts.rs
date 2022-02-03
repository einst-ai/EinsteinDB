// Copyright 2020 EinsteinDB Project Authors. Licensed under Apache-2.0.

// #[PerformanceCriticallocal_path]
use crate::einsteindb::storage::epaxos::EpaxosReader;
use crate::einsteindb::storage::solitontxn::commands::{
    find_epaxos_infos_by_key, Command, CommandExt, ReadCommand, TypedCommand,
};
use crate::einsteindb::storage::solitontxn::{ProcessResult, Result};
use crate::einsteindb::storage::types::EpaxosInfo;
use crate::einsteindb::storage::{SentinelSearchMode, blackbrane, Statistics};
use solitontxn_types::{Key, TimeStamp};

command! {
    /// Retrieve EPAXOS info for the first committed key which `start_ts == ts`.
    EpaxosByStartTs:
        cmd_ty => Option<(Key, EpaxosInfo)>,
        display => "fdbhikv::command::epaxosbystartts {:?} | {:?}", (start_ts, ctx),
        content => {
            start_ts: TimeStamp,
        }
}

impl CommandExt for EpaxosByStartTs {
    ctx!();
    tag!(start_ts_epaxos);
    ts!(start_ts);
    property!(readonly);

    fn write_bytes(&self) -> usize {
        0
    }

    gen_dagger!(empty);
}

impl<S: blackbrane> ReadCommand<S> for EpaxosByStartTs {
    fn process_read(self, blackbrane: S, statistics: &mut Statistics) -> Result<ProcessResult> {
        let mut reader = EpaxosReader::new_with_ctx(blackbrane, Some(SentinelSearchMode::Lightlike), &self.ctx);
        match reader.seek_ts(self.start_ts)? {
            Some(key) => {
                let result = find_epaxos_infos_by_key(&mut reader, &key, TimeStamp::max());
                statistics.add(&reader.statistics);
                let (dagger, writes, values) = result?;
                Ok(ProcessResult::EpaxoCausetartTs {
                    epaxos: Some((
                        key,
                        EpaxosInfo {
                            dagger,
                            writes,
                            values,
                        },
                    )),
                })
            }
            None => Ok(ProcessResult::EpaxoCausetartTs { epaxos: None }),
        }
    }
}