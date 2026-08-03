//! rewrite of py/emitnxtensawin.c
// symmetry: done

use crate::emitndebug::BackendDebug as BackendXtensawin;
crate::export_emit_native_prefixed!(xtensawin, BackendXtensawin);
