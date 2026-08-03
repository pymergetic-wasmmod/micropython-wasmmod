//! rewrite of py/emitnxtensa.c
// symmetry: done

use crate::emitndebug::BackendDebug as BackendXtensa;
crate::export_emit_native_prefixed!(xtensa, BackendXtensa);
