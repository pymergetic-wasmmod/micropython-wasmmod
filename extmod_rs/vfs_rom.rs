//! rewrite of extmod/vfs_rom.c + extmod/vfs_rom.h
// symmetry: done

use py_rs::argcheck;
use py_rs::builtinimport::ImportStat;
use py_rs::malloc;
use py_rs::map::{self, MapElem};
use py_rs::mpconfig;
use py_rs::mperrno;
use py_rs::obj::{self, BufferInfo, MakeNewFn, Obj, ObjBase, ObjType, TYPE_FLAG_BINDS_SELF, TYPE_FLAG_BUILTIN_FUN};
use py_rs::objdict::{self, ObjDict};
use py_rs::objpolyiter;
use py_rs::objstr;
use py_rs::objtuple;
use py_rs::qstr;
use py_rs::raise::{self, MpRaise};

use crate::vfs_rom_file;

const ROMFS_SIZE_MIN: usize = 4;
const ROMFS_HEADER_BYTE0: u8 = 0x80 | b'R';
const ROMFS_HEADER_BYTE1: u8 = 0x80 | b'M';
const ROMFS_HEADER_BYTE2: u8 = b'1';

const ROMFS_RECORD_KIND_UNUSED: u32 = 0;
const ROMFS_RECORD_KIND_PADDING: u32 = 1;
const ROMFS_RECORD_KIND_DATA_VERBATIM: u32 = 2;
const ROMFS_RECORD_KIND_DATA_POINTER: u32 = 3;
const ROMFS_RECORD_KIND_DIRECTORY: u32 = 4;
const ROMFS_RECORD_KIND_FILE: u32 = 5;
const ROMFS_RECORD_KIND_FILESYSTEM: u32 = 0x14a6b1;

pub const MP_S_IFDIR: i32 = 0x4000;
pub const MP_S_IFREG: i32 = 0x8000;

type RecordKind = u32;

#[repr(C)]
pub struct ObjVfsRom {
    pub base: ObjBase,
    pub memory: Obj,
    pub filesystem: *const u8,
    pub filesystem_end: *const u8,
}

#[repr(C)]
pub struct VfsProto {
    pub import_stat: fn(*const ObjVfsRom, &str) -> ImportStat,
}

fn vfs_ptr(o: Obj) -> *mut ObjVfsRom {
    obj::as_ptr(o) as *mut ObjVfsRom
}

fn get_path_str(_self: &ObjVfsRom, path: Obj) -> String {
    objstr::str_get_str(path)
}

fn decode_uint_checked(ptr: &mut *const u8, ptr_max: *const u8, value_out: &mut usize) -> bool {
    let mut unum = 0usize;
    loop {
        if *ptr >= ptr_max {
            return false;
        }
        let val = unsafe { **ptr };
        *ptr = unsafe { ptr.add(1) };
        unum = (unum << 7) | (val & 0x7f) as usize;
        if val & 0x80 == 0 {
            break;
        }
    }
    *value_out = unum;
    true
}

fn extract_record(
    fs: &mut *const u8,
    fs_next: &mut *const u8,
    fs_max: *const u8,
) -> RecordKind {
    let mut record_kind = 0usize;
    if !decode_uint_checked(fs, fs_max, &mut record_kind) {
        return ROMFS_RECORD_KIND_UNUSED;
    }
    let mut record_len = 0usize;
    if !decode_uint_checked(fs, fs_max, &mut record_len) {
        return ROMFS_RECORD_KIND_UNUSED;
    }
    *fs_next = unsafe { fs.add(record_len) };
    record_kind as RecordKind
}

fn extract_data(
    self_: &ObjVfsRom,
    mut fs: *const u8,
    fs_top: *const u8,
    size_out: Option<&mut usize>,
    data_out: Option<&mut *const u8>,
) -> Result<(), i32> {
    while fs < fs_top {
        let mut fs_next = fs;
        let record_kind = extract_record(&mut fs, &mut fs_next, fs_top);
        if record_kind == ROMFS_RECORD_KIND_UNUSED {
            break;
        } else if record_kind == ROMFS_RECORD_KIND_DATA_VERBATIM {
            if let Some(size_out) = size_out {
                *size_out = fs_next as usize - fs as usize;
            }
            if let Some(data_out) = data_out {
                *data_out = fs;
            }
            return Ok(());
        } else if record_kind == ROMFS_RECORD_KIND_DATA_POINTER {
            let mut size = 0usize;
            if !decode_uint_checked(&mut fs, fs_next, &mut size) {
                break;
            }
            let mut offset = 0usize;
            if !decode_uint_checked(&mut fs, fs_next, &mut offset) {
                break;
            }
            if let Some(size_out) = size_out {
                *size_out = size;
            }
            if let Some(data_out) = data_out {
                *data_out = unsafe { self_.filesystem.add(offset) };
            }
            return Ok(());
        } else {
            fs = fs_next;
        }
    }
    Err(-mperrno::EIO)
}

/// `mp_vfs_rom_search_filesystem`
pub fn search_filesystem(
    self_: &ObjVfsRom,
    path: &str,
    size_out: Option<&mut usize>,
    data_out: Option<&mut *const u8>,
) -> ImportStat {
    let mut fs = self_.filesystem;
    let mut fs_top = self_.filesystem_end;
    let mut path = path;
    let mut path_len = path.len();
    if path.starts_with('/') {
        path = &path[1..];
        path_len -= 1;
    }
    while path_len > 0 && fs < fs_top {
        let mut fs_next = fs;
        let record_kind = extract_record(&mut fs, &mut fs_next, fs_top);
        if record_kind == ROMFS_RECORD_KIND_UNUSED {
            return ImportStat::NoExist;
        } else if record_kind == ROMFS_RECORD_KIND_DIRECTORY || record_kind == ROMFS_RECORD_KIND_FILE {
            let mut name_len = 0usize;
            if !decode_uint_checked(&mut fs, fs_next, &mut name_len) {
                return ImportStat::NoExist;
            }
            if (name_len == path_len || (name_len < path_len && path.as_bytes()[name_len] == b'/'))
                && fs as usize + name_len <= fs_next as usize
                && &path.as_bytes()[..name_len] == unsafe { std::slice::from_raw_parts(fs, name_len) }
            {
                fs = unsafe { fs.add(name_len) };
                fs_top = fs_next;
                path = &path[name_len..];
                path_len -= name_len;
                if record_kind == ROMFS_RECORD_KIND_DIRECTORY {
                    if path.starts_with('/') {
                        path = &path[1..];
                        path_len -= 1;
                    }
                } else {
                    if path_len != 0 {
                        return ImportStat::NoExist;
                    }
                    if extract_data(self_, fs, fs_top, size_out, data_out).is_err() {
                        return ImportStat::NoExist;
                    }
                    return ImportStat::File;
                }
            } else {
                fs = fs_next;
            }
        } else {
            fs = fs_next;
        }
    }
    if path_len == 0 {
        if let Some(size_out) = size_out {
            *size_out = fs_top as usize - fs as usize;
        }
        if let Some(data_out) = data_out {
            *data_out = fs;
        }
        return ImportStat::Dir;
    }
    ImportStat::NoExist
}

fn make_new(_type_in: &ObjType, n_args: usize, n_kw: usize, args: &[Obj]) -> Obj {
    argcheck::check_num(n_args, n_kw, 1, 1, false);
    let o = malloc::new_obj::<ObjVfsRom>().expect("VfsRom");
    unsafe {
        (*o).base.type_ = type_vfs_rom();
        (*o).memory = args[0];
    }

    let mut bufinfo = BufferInfo::default();
    obj::get_buffer_raise(args[0], &mut bufinfo, obj::BUFFER_READ);
    if bufinfo.len < ROMFS_SIZE_MIN {
        raise::raise(MpRaise::OSError(mperrno::ENODEV));
    }
    let filesystem = bufinfo.buf as *const u8;

    unsafe {
        if !((*filesystem == ROMFS_HEADER_BYTE0
            && *filesystem.add(1) == ROMFS_HEADER_BYTE1
            && *filesystem.add(2) == ROMFS_HEADER_BYTE2))
        {
            raise::raise(MpRaise::OSError(mperrno::ENODEV));
        }
    }

    let mut fs = filesystem;
    let mut fs_end = filesystem;
    let record_kind = extract_record(&mut fs, &mut fs_end, unsafe {
        filesystem.add(bufinfo.len)
    });
    if record_kind != ROMFS_RECORD_KIND_FILESYSTEM {
        raise::raise(MpRaise::OSError(mperrno::ENODEV));
    }

    if fs_end > unsafe { filesystem.add(bufinfo.len) } {
        raise::raise(MpRaise::OSError(mperrno::ENODEV));
    }

    unsafe {
        (*o).filesystem = fs;
        (*o).filesystem_end = fs_end;
        obj::from_ptr(o as *const ObjVfsRom as *const ())
    }
}

fn mount(_self_in: Obj, _readonly: Obj, mkfs: Obj) -> Obj {
    if obj::is_true(mkfs) {
        raise::raise(MpRaise::OSError(mperrno::EPERM));
    }
    obj::CONST_NONE
}

fn open(self_in: Obj, path_in: Obj, mode_in: Obj) -> Obj {
    vfs_rom_file::open(self_in, path_in, mode_in)
}

fn chdir(self_in: Obj, path_in: Obj) -> Obj {
    let self_ = unsafe { &*vfs_ptr(self_in) };
    let path = get_path_str(self_, path_in);
    if path == "/" {
        return obj::CONST_NONE;
    }
    raise::raise(MpRaise::OSError(mperrno::EOPNOTSUPP));
}

fn getcwd(_self_in: Obj) -> Obj {
    objstr::new_str(b"/")
}

#[repr(C)]
struct IlistdirIter {
    base: ObjBase,
    iternext: py_rs::obj::IterNextFn,
    vfs_rom: *const ObjVfsRom,
    is_str: bool,
    index: *const u8,
    index_top: *const u8,
}

fn ilistdir_iternext(self_in: Obj) -> Obj {
    let self_ = unsafe { &mut *(obj::as_ptr(self_in) as *mut IlistdirIter) };
    while self_.index < self_.index_top {
        let mut index_next = self_.index;
        let record_kind = extract_record(&mut self_.index, &mut index_next, self_.index_top);
        let (type_, name_len, data_len) = if record_kind == ROMFS_RECORD_KIND_UNUSED {
            self_.index = self_.index_top;
            break;
        } else if record_kind == ROMFS_RECORD_KIND_DIRECTORY || record_kind == ROMFS_RECORD_KIND_FILE {
            let mut name_len = 0usize;
            if !decode_uint_checked(&mut self_.index, index_next, &mut name_len) {
                self_.index = self_.index_top;
                break;
            }
            let data_len = if record_kind == ROMFS_RECORD_KIND_DIRECTORY {
                index_next as usize - self_.index as usize - name_len
            } else {
                let mut data_value = self_.index;
                let mut size = 0usize;
                if extract_data(
                    unsafe { &*self_.vfs_rom },
                    unsafe { self_.index.add(name_len) },
                    index_next,
                    Some(&mut size),
                    Some(&mut data_value),
                )
                .is_err()
                {
                    break;
                }
                size
            };
            let type_ = if record_kind == ROMFS_RECORD_KIND_DIRECTORY {
                MP_S_IFDIR
            } else {
                MP_S_IFREG
            };
            (type_, name_len, data_len)
        } else {
            self_.index = index_next;
            continue;
        };

        let name_str = self_.index;
        self_.index = index_next;

        let name_obj = if self_.is_str {
            objstr::new_str(unsafe { std::slice::from_raw_parts(name_str, name_len) })
        } else {
            objstr::new_bytes(unsafe { std::slice::from_raw_parts(name_str, name_len) })
        };

        return objtuple::new_tuple(
            4,
            Some(&[
                name_obj,
                obj::new_small_int(type_ as isize),
                obj::new_small_int(0),
                obj::new_small_int(data_len as isize),
            ]),
        );
    }
    obj::OBJ_STOP_ITERATION
}

fn ilistdir(self_in: Obj, path_in: Obj) -> Obj {
    let self_ = unsafe { &*vfs_ptr(self_in) };
    let is_str = obj::is_str(path_in);
    let path = get_path_str(self_, path_in);
    let mut size = 0usize;
    let mut index = core::ptr::null();
    if search_filesystem(self_, &path, Some(&mut size), Some(&mut index)) != ImportStat::Dir {
        raise::raise(MpRaise::OSError(mperrno::ENOENT));
    }
    let o = malloc::new_obj::<IlistdirIter>().expect("VfsRom ilistdir");
    unsafe {
        (*o).base.type_ = objpolyiter::type_polymorph_iter();
        (*o).iternext = ilistdir_iternext;
        (*o).vfs_rom = self_;
        (*o).is_str = is_str;
        (*o).index = index;
        (*o).index_top = unsafe { index.add(size) };
        obj::from_ptr(o as *const IlistdirIter as *const ())
    }
}

fn stat(self_in: Obj, path_in: Obj) -> Obj {
    let self_ = unsafe { &*vfs_ptr(self_in) };
    let path = get_path_str(self_, path_in);
    let mut file_size = 0usize;
    let mut file_data = core::ptr::null();
    let stat = search_filesystem(
        self_,
        &path,
        Some(&mut file_size),
        Some(&mut file_data),
    );
    if stat == ImportStat::NoExist {
        raise::raise(MpRaise::OSError(mperrno::ENOENT));
    }
    objtuple::new_tuple(
        10,
        Some(&[
            obj::new_small_int(if stat == ImportStat::File {
                MP_S_IFREG
            } else {
                MP_S_IFDIR
            } as isize),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(file_size as isize),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(0),
        ]),
    )
}

fn statvfs(self_in: Obj, _path_in: Obj) -> Obj {
    let self_ = unsafe { &*vfs_ptr(self_in) };
    let filesystem_len = unsafe { self_.filesystem_end.offset_from(self_.filesystem) } as usize;
    objtuple::new_tuple(
        10,
        Some(&[
            obj::new_small_int(1),
            obj::new_small_int(0),
            obj::new_small_int(filesystem_len as isize),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(0),
            obj::new_small_int(32767),
        ]),
    )
}

fn import_stat(self_: *const ObjVfsRom, path: &str) -> ImportStat {
    search_filesystem(unsafe { &*self_ }, path, None, None)
}

type BuiltinFn1 = fn(Obj) -> Obj;
type BuiltinFn2 = fn(Obj, Obj) -> Obj;
type BuiltinFn3 = fn(Obj, Obj, Obj) -> Obj;

#[repr(C)]
struct ObjFunBuiltin1 {
    base: ObjBase,
    fun: BuiltinFn1,
}
#[repr(C)]
struct ObjFunBuiltin2 {
    base: ObjBase,
    fun: BuiltinFn2,
}
#[repr(C)]
struct ObjFunBuiltin3 {
    base: ObjBase,
    fun: BuiltinFn3,
}

static mut F1: [*const (); 1] = [call1 as *const ()];
static mut F2: [*const (); 1] = [call2 as *const ()];
static mut F3: [*const (); 1] = [call3 as *const ()];
static TF1: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F1.as_ptr() },
};
static TF2: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F2.as_ptr() },
};
static TF3: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: TYPE_FLAG_BINDS_SELF | TYPE_FLAG_BUILTIN_FUN,
    name: 0,
    slot_index_make_new: 0,
    slot_index_print: 0,
    slot_index_call: 1,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 0,
    slot_index_parent: 0,
    slot_index_locals_dict: 0,
    slots: unsafe { F3.as_ptr() },
};

fn call1(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 1, 1, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin1)).fun)(a[0]) }
}
fn call2(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 2, 2, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin2)).fun)(a[0], a[1]) }
}
fn call3(s: Obj, n: usize, k: usize, a: &[Obj]) -> Obj {
    py_rs::argcheck::check_num(n, k, 3, 3, false);
    unsafe { ((*(obj::as_ptr(s) as *const ObjFunBuiltin3)).fun)(a[0], a[1], a[2]) }
}

fn mk1(f: BuiltinFn1) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin1>().expect("vfs_rom fn1");
    unsafe {
        (*o).base.type_ = &TF1;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin1 as *const ())
    }
}
fn mk2(f: BuiltinFn2) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin2>().expect("vfs_rom fn2");
    unsafe {
        (*o).base.type_ = &TF2;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin2 as *const ())
    }
}
fn mk3(f: BuiltinFn3) -> Obj {
    let o = malloc::new_obj::<ObjFunBuiltin3>().expect("vfs_rom fn3");
    unsafe {
        (*o).base.type_ = &TF3;
        (*o).fun = f;
        obj::from_ptr(o as *const ObjFunBuiltin3 as *const ())
    }
}

static VFS_ROM_PROTO: VfsProto = VfsProto { import_stat };

static mut VFS_ROM_SLOTS: [*const (); 3] = [core::ptr::null(); 3];
static mut TYPE_VFS_ROM: ObjType = ObjType {
    base: ObjBase { type_: core::ptr::null() },
    flags: obj::TYPE_FLAG_NONE,
    name: 0,
    slot_index_make_new: 1,
    slot_index_print: 0,
    slot_index_call: 0,
    slot_index_unary_op: 0,
    slot_index_binary_op: 0,
    slot_index_attr: 0,
    slot_index_subscr: 0,
    slot_index_iter: 0,
    slot_index_buffer: 0,
    slot_index_protocol: 2,
    slot_index_parent: 0,
    slot_index_locals_dict: 3,
    slots: unsafe { VFS_ROM_SLOTS.as_ptr() },
};

static TYPE_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn init_type() {
    TYPE_INIT.get_or_init(|| {
        let mut table = vec![
            MapElem {
                key: obj::new_qstr(qstr::from_str("mount")),
                value: mk3(mount),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("umount")),
                value: mk1(obj::identity),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("open")),
                value: mk3(open),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("chdir")),
                value: mk2(chdir),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("getcwd")),
                value: mk1(getcwd),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("ilistdir")),
                value: mk2(ilistdir),
            },
            MapElem {
                key: obj::new_qstr(qstr::from_str("stat")),
                value: mk2(stat),
            },
        ];
        if mpconfig::PY_OS_STATVFS {
            table.push(MapElem {
                key: obj::new_qstr(qstr::from_str("statvfs")),
                value: mk2(statvfs),
            });
        }
        let ptr =
            obj::malloc_helper(core::mem::size_of::<ObjDict>(), objdict::type_dict()) as *mut ObjDict;
        unsafe {
            map::init_fixed_table(&mut (*ptr).map, table);
            VFS_ROM_SLOTS[0] = make_new as MakeNewFn as *const ();
            VFS_ROM_SLOTS[1] = &VFS_ROM_PROTO as *const VfsProto as *const ();
            VFS_ROM_SLOTS[2] = obj::from_ptr(ptr as *const ObjDict as *const ()).0 as *const ();
            TYPE_VFS_ROM.name = qstr::from_str("VfsRom");
        }
    });
}

pub fn type_vfs_rom() -> &'static ObjType {
    init_type();
    unsafe { &TYPE_VFS_ROM }
}

pub fn enabled() -> bool {
    mpconfig::VFS_ROM && mpconfig::PY_VFS
}

pub fn import_stat_for(obj_in: Obj, path: &str) -> ImportStat {
    let ptr = obj::as_ptr(obj_in) as *const ObjVfsRom;
    import_stat(ptr, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_uint(value: u32) -> Vec<u8> {
        let mut encoded = vec![(value & 0x7f) as u8];
        let mut value = value >> 7;
        while value != 0 {
            encoded.insert(0, (0x80 | (value & 0x7f)) as u8);
            value >>= 7;
        }
        encoded
    }

    fn pack(kind: u32, payload: &[u8]) -> Vec<u8> {
        let mut out = encode_uint(kind);
        out.extend_from_slice(&encode_uint(payload.len() as u32));
        out.extend_from_slice(payload);
        out
    }

    fn make_test_romfs() -> Vec<u8> {
        let mut root = Vec::new();
        root.extend_from_slice(&pack(
            ROMFS_RECORD_KIND_FILE,
            &{
                let mut p = encode_uint(8);
                p.extend_from_slice(b"test.txt");
                p.extend_from_slice(&pack(ROMFS_RECORD_KIND_DATA_VERBATIM, b"contents"));
                p
            },
        ));
        let mut image = vec![ROMFS_HEADER_BYTE0, ROMFS_HEADER_BYTE1, ROMFS_HEADER_BYTE2];
        let mut len = encode_uint(root.len() as u32);
        if (3 + len.len() + root.len()) % 2 == 1 {
            len.insert(0, 0x80);
        }
        image.extend_from_slice(&len);
        image.extend_from_slice(&root);
        image
    }

    fn romfs_from_bytes(data: &[u8]) -> ObjVfsRom {
        let mut fs = data.as_ptr();
        let mut fs_end = fs;
        let kind = extract_record(&mut fs, &mut fs_end, unsafe { data.as_ptr().add(data.len()) });
        assert_eq!(kind, ROMFS_RECORD_KIND_FILESYSTEM);
        ObjVfsRom {
            base: ObjBase { type_: core::ptr::null() },
            memory: obj::OBJ_NULL,
            filesystem: fs,
            filesystem_end: fs_end,
        }
    }

    #[test]
    fn search_finds_file_and_root() {
        let data = make_test_romfs();
        let vfs = romfs_from_bytes(&data);
        let mut size = 0usize;
        let mut ptr = core::ptr::null();
        assert_eq!(
            search_filesystem(&vfs, "", Some(&mut size), Some(&mut ptr)),
            ImportStat::Dir
        );
        assert_eq!(
            search_filesystem(&vfs, "test.txt", Some(&mut size), Some(&mut ptr)),
            ImportStat::File
        );
        assert_eq!(size, 8);
        unsafe {
            assert_eq!(
                std::slice::from_raw_parts(ptr, size),
                b"contents"
            );
        }
    }

    #[test]
    fn rejects_bad_header() {
        let data = b"xxx";
        let mut bufinfo = BufferInfo::default();
        bufinfo.buf = data.as_ptr() as *mut u8;
        bufinfo.len = data.len();
        assert!(data.len() < ROMFS_SIZE_MIN || data[0] != ROMFS_HEADER_BYTE0);
    }
}
