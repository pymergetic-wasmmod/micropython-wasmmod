//! rewrite of py/map.c
// symmetry: done

use crate::mpconfig;
use crate::obj::{self, Obj};
use crate::qstr;
use crate::runtime;
use crate::runtime0::UnaryOp;

/// Map element (`mp_map_elem_t`).
#[derive(Copy, Clone, Debug)]
pub struct MapElem {
    pub key: Obj,
    pub value: Obj,
}

impl Default for MapElem {
    fn default() -> Self {
        Self { key: obj::OBJ_NULL, value: obj::OBJ_NULL }
    }
}

/// Object map (`mp_map_t`).
#[derive(Debug, Default, Clone)]
pub struct Map {
    pub all_keys_are_qstrs: bool,
    pub is_fixed: bool,
    pub is_ordered: bool,
    pub used: usize,
    pub alloc: usize,
    pub table: Vec<MapElem>,
}

/// Lookup mode (`mp_map_lookup_kind_t`).
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LookupKind {
    Lookup = 0,
    AddIfNotFound = 1,
    RemoveIfFound = 2,
    AddIfNotFoundOrRemoveIfFound = 3,
}

const HASH_ALLOC_SIZES: &[u16] = &[
    0, 2, 4, 6, 8, 10, 12, 17, 23, 29, 37, 47, 59, 73, 97, 127, 167, 223, 293, 389, 521, 691, 919,
    1223, 1627, 2161, 3229, 4831, 7243, 10861, 16273, 24407, 36607, 54907,
];

fn hash_alloc_gte(x: usize) -> usize {
    for &size in HASH_ALLOC_SIZES {
        if size as usize >= x {
            return size as usize;
        }
    }
    (x + x / 2) | 1
}

pub fn slot_is_filled(map: &Map, pos: usize) -> bool {
    assert!(pos < map.alloc);
    let key = map.table[pos].key;
    key != obj::OBJ_NULL && key != obj::OBJ_SENTINEL
}

pub fn init(map: &mut Map, n: usize) {
    map.alloc = if n == 0 { 0 } else { n };
    map.table = if n == 0 { Vec::new() } else { vec![MapElem::default(); n] };
    map.used = 0;
    map.all_keys_are_qstrs = true;
    map.is_fixed = false;
    map.is_ordered = false;
}

pub fn init_fixed_table(map: &mut Map, table: Vec<MapElem>) {
    map.alloc = table.len();
    map.used = table.len();
    map.all_keys_are_qstrs = true;
    map.is_fixed = true;
    map.is_ordered = true;
    map.table = table;
}

pub fn deinit(map: &mut Map) {
    if !map.is_fixed {
        map.table.clear();
    }
    map.used = 0;
    map.alloc = 0;
}

pub fn clear(map: &mut Map) {
    if !map.is_fixed {
        map.table.clear();
    }
    map.alloc = 0;
    map.used = 0;
    map.all_keys_are_qstrs = true;
    map.is_fixed = false;
}

fn rehash(map: &mut Map) {
    let old_table = std::mem::take(&mut map.table);
    let old_alloc = map.alloc;
    map.alloc = hash_alloc_gte(map.alloc + 1);
    map.used = 0;
    map.all_keys_are_qstrs = true;
    map.table = vec![MapElem::default(); map.alloc];
    for elem in old_table.into_iter().take(old_alloc) {
        if elem.key != obj::OBJ_NULL && elem.key != obj::OBJ_SENTINEL {
            if let Some(slot) = lookup(map, elem.key, LookupKind::AddIfNotFound) {
                slot.value = elem.value;
            }
        }
    }
}

pub fn lookup(map: &mut Map, index: Obj, kind: LookupKind) -> Option<&mut MapElem> {
    assert!(!map.is_fixed || kind == LookupKind::Lookup);

    let compare_only_ptrs = if map.all_keys_are_qstrs {
        if obj::is_qstr(index) {
            true
        } else if obj::is_str(index) {
            false
        } else if kind != LookupKind::AddIfNotFound {
            return None;
        } else {
            false
        }
    } else {
        false
    };

    if map.is_ordered {
        return lookup_ordered(map, index, kind, compare_only_ptrs);
    }

    if map.alloc == 0 || map.table.is_empty() {
        if kind == LookupKind::AddIfNotFound {
            rehash(map);
        } else {
            return None;
        }
    }
    // Keep alloc consistent with the backing table (GC / reinit edge cases).
    if map.alloc != map.table.len() {
        map.alloc = map.table.len();
        if map.alloc == 0 {
            return None;
        }
    }

    let hash = if obj::is_qstr(index) {
        qstr::qstr_hash(obj::qstr_value(index)).unwrap_or(0)
    } else {
        obj::small_int_value(runtime::unary_op_obj(UnaryOp::Hash, index)) as usize
    };

    let mut pos = hash % map.alloc;
    let start_pos = pos;
    let mut avail_slot = None;
    loop {
        let slot_key = map.table[pos].key;
        if slot_key == obj::OBJ_NULL {
            if kind == LookupKind::AddIfNotFound {
                map.used += 1;
                let slot = avail_slot.unwrap_or(pos);
                map.table[slot].key = index;
                map.table[slot].value = obj::OBJ_NULL;
                if !obj::is_qstr(index) {
                    map.all_keys_are_qstrs = false;
                }
                return Some(&mut map.table[slot]);
            }
            return None;
        } else if slot_key == obj::OBJ_SENTINEL {
            avail_slot.get_or_insert(pos);
        } else if slot_key == index || (!compare_only_ptrs && obj::equal(slot_key, index)) {
            if kind == LookupKind::RemoveIfFound {
                map.used -= 1;
                if map.table[(pos + 1) % map.alloc].key == obj::OBJ_NULL {
                    map.table[pos].key = obj::OBJ_NULL;
                } else {
                    map.table[pos].key = obj::OBJ_SENTINEL;
                }
            }
            return Some(&mut map.table[pos]);
        }

        pos = (pos + 1) % map.alloc;
        if pos == start_pos {
            if kind == LookupKind::AddIfNotFound {
                if let Some(slot) = avail_slot {
                    map.used += 1;
                    map.table[slot].key = index;
                    map.table[slot].value = obj::OBJ_NULL;
                    if !obj::is_qstr(index) {
                        map.all_keys_are_qstrs = false;
                    }
                    return Some(&mut map.table[slot]);
                }
                rehash(map);
                pos = hash % map.alloc;
                avail_slot = None;
            } else {
                return None;
            }
        }
    }
}

fn lookup_ordered(map: &mut Map, index: Obj, kind: LookupKind, compare_only_ptrs: bool) -> Option<&mut MapElem> {
    for i in 0..map.used {
        let key = map.table[i].key;
        if key == index || (!compare_only_ptrs && obj::equal(key, index)) {
            if mpconfig::PY_COLLECTIONS_ORDEREDDICT && kind == LookupKind::RemoveIfFound {
                let value = map.table[i].value;
                map.used -= 1;
                map.table.copy_within(i + 1..=map.used, i);
                map.table[map.used].key = obj::OBJ_NULL;
                map.table[map.used].value = value;
                return Some(&mut map.table[map.used]);
            }
            return Some(&mut map.table[i]);
        }
    }
    if kind != LookupKind::AddIfNotFound {
        return None;
    }
    if mpconfig::PY_COLLECTIONS_ORDEREDDICT {
        if map.used == map.alloc {
            map.alloc += 4;
            map.table.resize(map.alloc, MapElem::default());
        }
        map.table[map.used].key = index;
        map.table[map.used].value = obj::OBJ_NULL;
        if !obj::is_qstr(index) {
            map.all_keys_are_qstrs = false;
        }
        map.used += 1;
        return Some(&mut map.table[map.used - 1]);
    }
    None
}

/// Set type (`mp_set_t`).
#[derive(Debug, Default)]
pub struct Set {
    pub alloc: usize,
    pub used: usize,
    pub table: Vec<Obj>,
}

pub fn set_slot_is_filled(set: &Set, pos: usize) -> bool {
    set.table[pos] != obj::OBJ_NULL && set.table[pos] != obj::OBJ_SENTINEL
}

pub fn set_init(set: &mut Set, n: usize) {
    set.alloc = n;
    set.used = 0;
    set.table = vec![obj::OBJ_NULL; n];
}

fn set_rehash(set: &mut Set) {
    let old_table = std::mem::take(&mut set.table);
    let old_alloc = set.alloc;
    set.alloc = hash_alloc_gte(set.alloc + 1);
    set.used = 0;
    set.table = vec![obj::OBJ_NULL; set.alloc];
    for elem in old_table.into_iter().take(old_alloc) {
        if elem != obj::OBJ_NULL && elem != obj::OBJ_SENTINEL {
            set_lookup(set, elem, LookupKind::AddIfNotFound);
        }
    }
}

pub fn set_lookup(set: &mut Set, index: Obj, kind: LookupKind) -> Obj {
    if set.alloc == 0 {
        if (kind as u8) & (LookupKind::AddIfNotFound as u8) != 0 {
            set_rehash(set);
        } else {
            return obj::OBJ_NULL;
        }
    }
    let hash = obj::small_int_value(runtime::unary_op_obj(UnaryOp::Hash, index)) as usize;
    let mut pos = hash % set.alloc;
    let start_pos = pos;
    let mut avail_slot = None;
    loop {
        let elem = set.table[pos];
        if elem == obj::OBJ_NULL {
            if (kind as u8) & (LookupKind::AddIfNotFound as u8) != 0 {
                let slot = avail_slot.unwrap_or(pos);
                set.used += 1;
                set.table[slot] = index;
                return index;
            }
            return obj::OBJ_NULL;
        } else if elem == obj::OBJ_SENTINEL {
            avail_slot.get_or_insert(pos);
        } else if obj::equal(elem, index) {
            if (kind as u8) & (LookupKind::RemoveIfFound as u8) != 0 {
                set.used -= 1;
                if set.table[(pos + 1) % set.alloc] == obj::OBJ_NULL {
                    set.table[pos] = obj::OBJ_NULL;
                } else {
                    set.table[pos] = obj::OBJ_SENTINEL;
                }
            }
            return elem;
        }
        pos = (pos + 1) % set.alloc;
        if pos == start_pos {
            if (kind as u8) & (LookupKind::AddIfNotFound as u8) != 0 {
                if let Some(slot) = avail_slot {
                    set.used += 1;
                    set.table[slot] = index;
                    return index;
                }
                set_rehash(set);
                pos = hash % set.alloc;
                avail_slot = None;
            } else {
                return obj::OBJ_NULL;
            }
        }
    }
}

pub fn set_remove_first(set: &mut Set) -> Obj {
    for pos in 0..set.alloc {
        if set_slot_is_filled(set, pos) {
            let elem = set.table[pos];
            set.used -= 1;
            if set.table.get(pos + 1).copied().unwrap_or(obj::OBJ_NULL) == obj::OBJ_NULL {
                set.table[pos] = obj::OBJ_NULL;
            } else {
                set.table[pos] = obj::OBJ_SENTINEL;
            }
            return elem;
        }
    }
    obj::OBJ_NULL
}

pub fn set_clear(set: &mut Set) {
    set.table.clear();
    set.alloc = 0;
    set.used = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qstr;

    #[test]
    fn ordered_map_lookup_add() {
        let mut map = Map::default();
        map.is_ordered = true;
        map.alloc = 4;
        map.table = vec![MapElem::default(); 4];
        let key = obj::new_qstr(qstr::from_str("foo"));
        let slot = lookup(&mut map, key, LookupKind::AddIfNotFound).unwrap();
        slot.value = obj::new_small_int(42);
        let found = lookup(&mut map, key, LookupKind::Lookup).unwrap();
        assert_eq!(found.value, obj::new_small_int(42));
    }
}
