//! rewrite of shared/runtime/stdout_helpers.c
// symmetry: done

use py_rs::mphal;

/// `mp_hal_stdout_tx_strn_cooked` — translate `\n` to `\r\n`.
pub fn stdout_tx_strn_cooked(str: &str, len: usize) {
    let end = len.min(str.len());
    let bytes = &str.as_bytes()[..end];
    let mut last = 0usize;
    for (i, &byte) in bytes.iter().enumerate() {
        if byte == b'\n' {
            if i > last {
                mphal::stdout_tx_strn(
                    unsafe { std::str::from_utf8_unchecked(&bytes[last..i]) },
                    i - last,
                );
            }
            mphal::stdout_tx_strn("\r\n", 2);
            last = i + 1;
        }
    }
    if last < bytes.len() {
        mphal::stdout_tx_strn(
            unsafe { std::str::from_utf8_unchecked(&bytes[last..]) },
            bytes.len() - last,
        );
    }
}

/// `mp_hal_stdout_tx_str`.
pub fn stdout_tx_str(str: &str) {
    mphal::stdout_tx_str(str);
}
