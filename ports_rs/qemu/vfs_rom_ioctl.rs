//! rewrite of ports/qemu/vfs_rom_ioctl.c
// symmetry: done

use py_rs::mperrno;
use py_rs::obj::Obj;

pub const ROM_IOCTL_GET_NUMBER_OF_SEGMENTS: i32 = 1;
pub const ROM_IOCTL_GET_SEGMENT: i32 = 2;

static ROMFS_PART0: [u8; 0] = [];
static ROMFS_PART1: [u8; 0] = [];

/// `mp_vfs_rom_ioctl`
pub fn vfs_rom_ioctl(cmd: i32, args: &[Obj]) -> Obj {
    if !py_rs::mpconfig::VFS_ROM_IOCTL {
        return py_rs::obj::new_small_int(-(mperrno::EINVAL as isize));
    }
    if cmd == ROM_IOCTL_GET_NUMBER_OF_SEGMENTS {
        return py_rs::obj::new_small_int(segments().len() as isize);
    }
    if args.is_empty() {
        return py_rs::obj::new_small_int(-(mperrno::EINVAL as isize));
    }
    let id = py_rs::obj::get_int(args[0]) as usize;
    let segs = segments();
    if id >= segs.len() {
        return py_rs::obj::new_small_int(-(mperrno::EINVAL as isize));
    }
    if cmd == ROM_IOCTL_GET_SEGMENT {
        return memoryview_for(segs[id]);
    }
    py_rs::obj::new_small_int(-(mperrno::EINVAL as isize))
}

fn segments() -> Vec<&'static [u8]> {
    vec![&ROMFS_PART0, &ROMFS_PART1]
}

fn memoryview_for(data: &[u8]) -> Obj {
    objstr::from_bytes(data)
}

mod objstr {
    use py_rs::obj::Obj;
    use py_rs::objstr;
    pub fn from_bytes(data: &[u8]) -> Obj {
        objstr::new_str(data)
    }
}
