//! Copies the net named by `EVALFILE` (default: the current net in `nets/`) into OUT_DIR so
//! `nnue` builds can embed it. Without the `nnue` feature, or with `EVALFILE=""`, an empty
//! file is written (no net).

use std::{env, fs, path::PathBuf};

/// Net embedded by default; CHANGELOG.md records which net each build uses.
const DEFAULT_NET: &str = "nets/aqueval-gen1-7ae6f8c0.nnue";

fn main() {
    println!("cargo:rerun-if-env-changed=EVALFILE");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("embedded.nnue");
    let net = match env::var("EVALFILE") {
        Ok(p) => Some(p).filter(|p| !p.is_empty()),
        Err(_) => Some(format!("{}/{DEFAULT_NET}", env::var("CARGO_MANIFEST_DIR").unwrap())),
    };
    match net {
        Some(path) if env::var_os("CARGO_FEATURE_NNUE").is_some() => {
            println!("cargo:rerun-if-changed={path}");
            fs::copy(&path, &out).unwrap_or_else(|e| panic!("EVALFILE {path}: {e}"));
        }
        _ => fs::write(&out, []).unwrap(),
    }
}
