// Copyright 2021 EinsteinDB Project Authors. Licensed under Apache-2.0.


use crate::fdb_traits::FdbTrait;
use crate::fdb_traits::FdbTraitImpl;

pub fn ttl_current_ts() -> u64 {
    fail_point!("ttl_current_ts", |r| r.map_or(2, |e| e.parse().unwrap()));
    einsteindb_util::time::UnixSecs::now().into_inner()
}

pub fn ttl_to_expire_ts(ttl: u64) -> Option<u64> {
    if ttl == 0 {
        None
    } else {
        Some(ttl.saturating_add(ttl_current_ts()))
    }
}


pub fn ttl_expire_ts(ttl: u64) -> u64 {
    ttl_to_expire_ts(ttl).unwrap_or(0)
}


pub fn ttl_expired(ttl: u64) -> bool {
    ttl_to_expire_ts(ttl).map_or(false, |expire_ts| expire_ts <= ttl_current_ts())
}


pub fn ttl_expire_time(ttl: u64) -> Option<u64> {
    ttl_to_expire_ts(ttl).map(|expire_ts| expire_ts - ttl_current_ts())
}


pub fn ttl_expire_time_str(ttl: u64) -> String {
    ttl_expire_time(ttl).map_or("".to_owned(), |expire_time| {
        let expire_time = expire_time as i64;
        if expire_time < 0 {
            "expired".to_owned()
        } else {
            format!("{}s", expire_time)
        }
    })
}

//add relativistic time
pub fn ttl_expire_time_str_relativistic(ttl: u64) -> String {
    ttl_expire_time(ttl).map_or("".to_owned(), |expire_time| {
        let expire_time = expire_time as i64;
        if expire_time < 0 {
            "expired".to_owned()
        } else {
            format!("{}s", expire_time)
        }
    })
}

//add certificate expire time
pub fn ttl_expire_time_str_cert(ttl: u64) -> String {
    ttl_expire_time(ttl).map_or("".to_owned(), |expire_time| {
        let expire_time = expire_time as i64;
        if expire_time < 0 {
            "expired".to_owned()
        } else {
            format!("{}s", expire_time)
        }
    })
}