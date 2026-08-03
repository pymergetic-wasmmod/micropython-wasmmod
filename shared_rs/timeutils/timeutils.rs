//! rewrite of shared/timeutils/timeutils.c + shared/timeutils/timeutils.h
// symmetry: done

use py_rs::mpconfig;
use py_rs::obj::{self, Obj, Uint};
use py_rs::objint;

pub type Timestamp = Uint;

pub const SECONDS_1970_TO_2000: i64 = 946_684_800;

const PREV_LEAP_DAY: Uint = (365 + 366 - (31 + 29)) as Uint;
const PREV_LEAP_YEAR: Uint = 1968;

const QC_BASE_DAY: Uint = 134_409;
const QC_LEAP_YEAR: Uint = 1600;
const QC_LEAP_DAYS: Uint = 89;

const DAYS_PER_400Y: Uint = 365 * 400 + 97;
const DAYS_PER_100Y: Uint = 365 * 100 + 24;
const DAYS_PER_4Y: Uint = 365 * 4 + 1;

static DAYS_SINCE_JAN1: [u16; 13] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334, 365];

#[derive(Copy, Clone, Debug, Default)]
pub struct StructTime {
    pub tm_year: u16,
    pub tm_mon: u8,
    pub tm_mday: u8,
    pub tm_hour: u8,
    pub tm_min: u8,
    pub tm_sec: u8,
    pub tm_wday: u8,
    pub tm_yday: u16,
}

type RelInt = Uint;

pub fn is_leap_year(year: Uint) -> bool {
    if mpconfig::TIME_SUPPORT_Y2100_AND_BEYOND || mpconfig::TIME_SUPPORT_Y1969_AND_BEFORE {
        year % 4 == 0 && year % 100 != 0 || year % 400 == 0
    } else {
        year % 4 == 0
    }
}

pub fn days_in_month(year: Uint, month: Uint) -> Uint {
    let mut mdays = DAYS_SINCE_JAN1[month as usize] - DAYS_SINCE_JAN1[(month - 1) as usize];
    if month == 2 && is_leap_year(year) {
        mdays += 1;
    }
    mdays as Uint
}

pub fn year_day(year: Uint, month: Uint, date: Uint) -> Uint {
    let mut yday = DAYS_SINCE_JAN1[(month - 1) as usize] + date as u16;
    if month >= 3 && is_leap_year(year) {
        yday += 1;
    }
    yday as Uint
}

pub fn seconds_since_1970_to_struct_time(seconds: Timestamp, tm: &mut StructTime) {
    let mut days = (seconds / 86400) as RelInt;
    let seconds = seconds % 86400;
    tm.tm_hour = (seconds / 3600) as u8;
    tm.tm_min = (seconds / 60 % 60) as u8;
    tm.tm_sec = (seconds % 60) as u8;

    let wday = (days + 3) % 7;
    tm.tm_wday = wday as u8;

    days += PREV_LEAP_DAY;

    let (base_year, qc_cycles, c_cycles, mut days) =
        if mpconfig::TIME_SUPPORT_Y2100_AND_BEYOND || mpconfig::TIME_SUPPORT_Y1969_AND_BEFORE {
            let mut days = days + QC_BASE_DAY;
            let qc_cycles = days / DAYS_PER_400Y;
            days %= DAYS_PER_400Y;
            let mut c_cycles = days / DAYS_PER_100Y;
            if c_cycles == 4 {
                c_cycles -= 1;
            }
            days -= c_cycles * DAYS_PER_100Y;
            (QC_LEAP_YEAR, qc_cycles, c_cycles, days)
        } else {
            (PREV_LEAP_YEAR, 0, 0, days)
        };

    let mut q_cycles = days / DAYS_PER_4Y;
    if mpconfig::TIME_SUPPORT_Y2100_AND_BEYOND || mpconfig::TIME_SUPPORT_Y1969_AND_BEFORE {
        if q_cycles == 25 {
            q_cycles -= 1;
        }
    }
    days -= q_cycles * DAYS_PER_4Y;

    let mut years = days / 365;
    if years == 4 {
        years -= 1;
    }
    days -= years * 365;

    tm.tm_year = (base_year + years + 4 * q_cycles + 100 * c_cycles + 400 * qc_cycles) as u16;

    static DAYS_IN_MONTH: [u8; 12] = [31, 30, 31, 30, 31, 31, 30, 31, 30, 31, 31, 29];
    let mut month = 0usize;
    while (DAYS_IN_MONTH[month] as Uint) <= days {
        days -= DAYS_IN_MONTH[month] as Uint;
        month += 1;
    }

    tm.tm_mon = (month + 2) as u8;
    if tm.tm_mon >= 12 {
        tm.tm_mon -= 12;
        tm.tm_year += 1;
    }
    tm.tm_mday = (days + 1) as u8;
    tm.tm_mon += 1;

    tm.tm_yday = year_day(tm.tm_year as Uint, tm.tm_mon as Uint, tm.tm_mday as Uint) as u16;
}

pub fn seconds_since_1970(
    year: Uint,
    month: Uint,
    date: Uint,
    hour: Uint,
    minute: Uint,
    second: Uint,
) -> Timestamp {
    let ref_year =
        if mpconfig::TIME_SUPPORT_Y2100_AND_BEYOND || mpconfig::TIME_SUPPORT_Y1969_AND_BEFORE {
            QC_LEAP_YEAR
        } else {
            PREV_LEAP_YEAR
        };
    let mut res = (year as i64 - 1970) * 365;
    res += (year.saturating_sub(ref_year + 1) / 4) as i64;
    if mpconfig::TIME_SUPPORT_Y2100_AND_BEYOND || mpconfig::TIME_SUPPORT_Y1969_AND_BEFORE {
        res -= (year.saturating_sub(ref_year + 1) / 100) as i64;
        res += (year.saturating_sub(ref_year + 1) / 400) as i64;
        res -= QC_LEAP_DAYS as i64;
    }
    res += (year_day(year, month, date) - 1) as i64;
    res *= 86400;
    res += (hour * 3600 + minute * 60 + second) as i64;
    res as Timestamp
}

pub fn mktime_1970(
    mut year: Uint,
    mut month: i32,
    mut mday: i32,
    mut hours: i32,
    mut minutes: i32,
    mut seconds: i32,
) -> Timestamp {
    minutes += seconds / 60;
    seconds %= 60;
    if seconds < 0 {
        seconds += 60;
        minutes -= 1;
    }

    hours += minutes / 60;
    minutes %= 60;
    if minutes < 0 {
        minutes += 60;
        hours -= 1;
    }

    mday += hours / 24;
    hours %= 24;
    if hours < 0 {
        hours += 24;
        mday -= 1;
    }

    month -= 1;
    year = (year as i32 + month / 12) as Uint;
    month %= 12;
    if month < 0 {
        month += 12;
        year = year.saturating_sub(1);
    }
    month += 1;

    while mday < 1 {
        if month == 1 {
            month = 12;
            year -= 1;
        } else {
            month -= 1;
        }
        mday += days_in_month(year, month as Uint) as i32;
    }
    while mday as Uint > days_in_month(year, month as Uint) {
        mday -= days_in_month(year, month as Uint) as i32;
        if month == 12 {
            month = 1;
            year += 1;
        } else {
            month += 1;
        }
    }

    seconds_since_1970(
        year,
        month as Uint,
        mday as Uint,
        hours as Uint,
        minutes as Uint,
        seconds as Uint,
    )
}

pub fn calc_weekday(y: i32, m: i32, d: i32) -> i32 {
    let y = if m < 3 { y - 1 } else { y - 2 };
    let mut d = d + 23 * m / 9 + 4 + y / 4;
    if mpconfig::TIME_SUPPORT_Y2100_AND_BEYOND || mpconfig::TIME_SUPPORT_Y1969_AND_BEFORE {
        d = d - y / 100 + y / 400;
    }
    (d + 6) % 7
}

pub fn obj_get_timestamp(o: Obj) -> Timestamp {
    objint::int_get_truncated(o) as Timestamp
}

pub fn obj_from_timestamp(t: Timestamp) -> Obj {
    obj::new_small_int(t as isize)
}

pub fn seconds_since_2000_to_struct_time(t: Timestamp, tm: &mut StructTime) {
    seconds_since_1970_to_struct_time(t + SECONDS_1970_TO_2000 as Uint, tm);
}

pub fn seconds_since_2000(
    year: Uint,
    month: Uint,
    date: Uint,
    hour: Uint,
    minute: Uint,
    second: Uint,
) -> Timestamp {
    seconds_since_1970(year, month, date, hour, minute, second)
        .saturating_sub(SECONDS_1970_TO_2000 as Uint)
}

pub fn mktime_2000(
    year: Uint,
    month: i32,
    mday: i32,
    hours: i32,
    minutes: i32,
    seconds: i32,
) -> Timestamp {
    mktime_1970(year, month, mday, hours, minutes, seconds)
        .saturating_sub(SECONDS_1970_TO_2000 as Uint)
}

/// Nanoseconds since 1970-01-01 → host epoch seconds (mirrors `timeutils.h`).
pub fn seconds_since_epoch_from_nanoseconds_since_1970(ns: i64) -> Timestamp {
    let secs = ns / 1_000_000_000;
    if mpconfig::EPOCH_IS_1970 {
        secs as Timestamp
    } else {
        (secs - SECONDS_1970_TO_2000) as Timestamp
    }
}

/// Host monotonic wall time → nanoseconds since 1970 for on-disk LFS attrs.
pub fn nanoseconds_since_epoch_to_nanoseconds_since_1970(ns: u64) -> u64 {
    if mpconfig::EPOCH_IS_1970 {
        ns
    } else {
        ns.saturating_add(SECONDS_1970_TO_2000 as u64 * 1_000_000_000)
    }
}

/// Pack nanoseconds-since-1970 as little-endian bytes for `LFS_ATTR_MTIME`.
pub fn lfs_mtime_bytes_from_now() -> [u8; 8] {
    let ns = nanoseconds_since_epoch_to_nanoseconds_since_1970(py_rs::mphal::time_ns());
    ns.to_le_bytes()
}

/// Decode `LFS_ATTR_MTIME` bytes to a host-epoch timestamp.
pub fn lfs_mtime_bytes_to_timestamp(buf: &[u8; 8]) -> Timestamp {
    seconds_since_epoch_from_nanoseconds_since_1970(u64::from_le_bytes(*buf) as i64)
}
