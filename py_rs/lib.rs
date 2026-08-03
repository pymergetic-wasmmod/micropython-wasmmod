//! MetalPython rewrite of MicroPython `py/`.
//! Shadow tree: `py_rs/`.
#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_mut,
    unused_assignments,
    unused_unsafe,
    non_snake_case,
    non_upper_case_globals,
    static_mut_refs,
    private_interfaces,
    unexpected_cfgs,
    unpredictable_function_pointer_comparisons,
    improper_ctypes,
    improper_ctypes_definitions,
    clippy::all
)]

pub mod argcheck;
pub mod asmarm;
pub mod asmbase;
pub mod asmrv32;
pub mod asmthumb;
pub mod asmx64;
pub mod asmx86;
pub mod asmxtensa;
pub mod bc;
pub mod bc0;
pub mod binary;
pub mod builtin;
pub mod builtinevex;
pub mod builtinhelp;
pub mod builtinimport;
pub mod compile;
pub mod cstack;
pub mod dynruntime;
pub mod emit;
pub mod emitbc;
pub mod emitcommon;
pub mod emitdispatch;
pub mod emitglue;
pub mod emitinlinerv32;
pub mod emitinlinethumb;
pub mod emitinlinextensa;
pub mod emitnarm;
pub mod emitnative;
pub mod emitndebug;
pub mod emitnrv32;
pub mod emitnthumb;
pub mod emitnx64;
pub mod emitnx86;
pub mod emitnxtensa;
pub mod emitnxtensawin;
pub mod formatfloat;
pub mod frozenmod;
pub mod gc;
pub mod grammar;
pub mod lexer;
pub mod malloc;
pub mod map;
pub mod misc;
pub mod modarray;
pub mod modbuiltins;
pub mod modcmath;
pub mod modcollections;
pub mod moderrno;
pub mod modgc;
pub mod modio;
pub mod modmath;
pub mod modmicropython;
pub mod modstring;
pub mod modstruct;
pub mod modsys;
pub mod modthread;
pub mod modweakref;
pub mod mpconfig;
pub mod mperrno;
pub mod mphal;
pub mod mpprint;
pub mod mpstate;
pub mod mpthread;
pub mod mpz;
pub mod nativeglue;
pub mod nlr;
pub mod nlraarch64;
pub mod nlrloong64;
pub mod nlrmips;
pub mod nlrpowerpc;
pub mod nlrrv32;
pub mod nlrrv64;
pub mod nlrsetjmp;
pub mod nlrthumb;
pub mod nlrx64;
pub mod nlrx86;
pub mod nlrxtensa;
pub mod obj;
pub mod objarray;
pub mod objattrtuple;
pub mod objbool;
pub mod objboundmeth;
pub mod objcell;
pub mod objclosure;
pub mod objcode;
pub mod objcomplex;
pub mod objdeque;
pub mod objdict;
pub mod objenumerate;
pub mod objexcept;
pub mod objfilter;
pub mod objfloat;
pub mod objfun;
pub mod objgenerator;
pub mod objgetitemiter;
pub mod objint;
pub mod objint_impl;
pub mod objint_longlong;
pub mod objint_mpz;
pub mod objlist;
pub mod objmap;
pub mod objmodule;
pub mod objnamedtuple;
pub mod objnone;
pub mod objobject;
pub mod objpolyiter;
pub mod objproperty;
pub mod objrange;
pub mod objreversed;
pub mod objringio;
pub mod objset;
pub mod objsingleton;
pub mod objslice;
pub mod objstr;
pub mod objstringio;
pub mod objstrunicode;
pub mod objtemplate;
pub mod objtuple;
pub mod objtype;
pub mod objzip;
pub mod opmethods;
pub mod pairheap;
pub mod parse;
pub mod parsenum;
pub mod parsenumbase;
pub mod persistentcode;
pub mod profile;
pub mod pystack;
pub mod qstr;
pub mod qstrdefs;
pub mod raise;
pub mod reader;
pub mod repl;
pub mod ringbuf;
pub mod runtime;
pub mod runtime0;
pub mod runtime_utils;
pub mod scheduler;
pub mod scope;
pub mod sequence;
pub mod showbc;
pub mod smallint;
pub mod stackctrl;
pub mod stream;
pub mod unicode;
pub mod vm;
pub mod vm_test;
pub mod vmentrytable;
pub mod vstr;
pub mod warning;
pub mod pm;
