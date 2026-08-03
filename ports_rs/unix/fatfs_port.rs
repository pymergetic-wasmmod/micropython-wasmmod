//! rewrite of ports/unix/fatfs_port.c
// symmetry: done

use std::time::{SystemTime, UNIX_EPOCH};

/// `get_fattime` — FatFS timestamp hook (FAT encoding of local time).
pub fn get_fattime() -> u32 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let dt = chrono_like_local(now);
    ((dt.year - 1980) as u32) << 25
        | (dt.mon as u32) << 21
        | (dt.mday as u32) << 16
        | (dt.hour as u32) << 11
        | (dt.min as u32) << 5
        | (dt.sec / 2) as u32
}

struct LocalTm {
    year: i32,
    mon: i32,
    mday: i32,
    hour: i32,
    min: i32,
    sec: i32,
}

/// Local-time breakdown without pulling in chrono (uses libc `localtime`).
fn chrono_like_local(secs: i64) -> LocalTm {
    unsafe {
        let t = secs as libc::time_t;
        let p = libc::localtime(&t);
        if p.is_null() {
            return LocalTm {
                year: 1980,
                mon: 1,
                mday: 1,
                hour: 0,
                min: 0,
                sec: 0,
            };
        }
        LocalTm {
            year: (*p).tm_year + 1900,
            mon: (*p).tm_mon + 1,
            mday: (*p).tm_mday,
            hour: (*p).tm_hour,
            min: (*p).tm_min,
            sec: (*p).tm_sec,
        }
    }
}
