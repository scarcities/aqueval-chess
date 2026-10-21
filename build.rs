//! Copies the net named by `EVALFILE` into OUT_DIR so `nnue` builds can embed it.
//! Without the `nnue` feature, or without EVALFILE, an empty file is written (no net).

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=EVALFILE");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("embedded.nnue");
    let net = env::var("EVALFILE").ok().filter(|p| !p.is_empty());
    match net {
        Some(path) if env::var_os("CARGO_FEATURE_NNUE").is_some() => {
            println!("cargo:rerun-if-changed={path}");
            fs::copy(&path, &out).unwrap_or_else(|e| panic!("EVALFILE {path}: {e}"));
        }
        _ => fs::write(&out, []).unwrap(),
    }
}
