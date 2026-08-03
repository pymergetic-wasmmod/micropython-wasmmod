//! rewrite of ports/unix/input.c + ports/unix/input.h
// symmetry: done

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

/// `prompt` — read a line from stdin with optional prompt text.
pub fn prompt(p: &str) -> Option<String> {
    print!("{p}");
    let _ = io::stdout().flush();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).ok()?;
    if line.ends_with('\n') {
        line.pop();
        if line.ends_with('\r') {
            line.pop();
        }
    }
    Some(line)
}

/// Load `~/.micropython.history` into port history buffer.
pub fn prompt_read_history() {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let path = PathBuf::from(home).join(".micropython.history");
    let Ok(data) = std::fs::read_to_string(path) else {
        return;
    };
    super::mphalport::load_history_lines(data.lines().map(str::to_owned).collect());
}

/// Write port history buffer to `~/.micropython.history`.
pub fn prompt_write_history() {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let path = PathBuf::from(home).join(".micropython.history");
    let lines = super::mphalport::take_history_lines();
    if lines.is_empty() {
        return;
    }
    let mut out = String::new();
    for line in lines {
        out.push_str(&line);
        out.push('\n');
    }
    let _ = std::fs::write(path, out);
}
