//! Console logging.
//!
//! Deliberately not a logging framework. Until stage 3 gives PID 1 a real log
//! handler, everything goes to the console the operator is already staring at.

use std::io::Write;

const PREFIX: &str = "[init]";

pub fn info(msg: &str) {
    emit("", msg);
}

pub fn warn(msg: &str) {
    emit(" warn:", msg);
}

pub fn error(msg: &str) {
    emit(" error:", msg);
}

fn emit(level: &str, msg: &str) {
    let mut err = std::io::stderr();
    let _ = writeln!(err, "{PREFIX}{level} {msg}");
    let _ = err.flush();
}
