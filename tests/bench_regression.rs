use std::process::Command;
use aqueval::bench::run_bench_with_writer;

/// Bench signature of the default build (NNUE with the net in `nets/`) and of the
/// handcrafted-eval build (`--no-default-features`).
#[cfg(feature = "nnue")]
const SIGNATURE: u64 = 680_520;
#[cfg(not(feature = "nnue"))]
const SIGNATURE: u64 = 912_599;

#[test]
fn test_bench_deterministic_signature_consecutive_runs() {
    let mut sink = std::io::sink();

    // Run 1
    let sig1 = run_bench_with_writer(Some(4), &mut sink);
    // Run 2
    let sig2 = run_bench_with_writer(Some(4), &mut sink);
    // Run 3
    let sig3 = run_bench_with_writer(Some(4), &mut sink);

    assert_eq!(sig1, sig2, "Bench signature must match across consecutive runs");
    assert_eq!(sig2, sig3, "Bench signature must match across consecutive runs");
    assert!(sig1 > 0, "Bench signature must be greater than zero");
}

#[test]
fn test_bench_default_depth_deterministic() {
    let mut sink = std::io::sink();

    let sig1 = run_bench_with_writer(None, &mut sink);
    let sig2 = run_bench_with_writer(None, &mut sink);

    assert_eq!(sig1, sig2, "Default bench signature must be deterministic");
    assert_eq!(sig1, SIGNATURE, "default bench signature");
}

#[test]
fn test_bench_cli_binary_execution() {
    let bin_path = env!("CARGO_BIN_EXE_aqueval");

    // Execute `aqueval bench`
    let output1 = Command::new(bin_path)
        .arg("bench")
        .output()
        .expect("Failed to execute aqueval bench");

    assert!(output1.status.success());
    let stdout1 = String::from_utf8(output1.stdout).expect("Valid UTF-8");
    assert!(stdout1.contains(&format!("Nodes searched  : {SIGNATURE}")));
    assert!(stdout1.contains("Total time (ms) :"));
    assert!(stdout1.contains("Nodes/second    :"));

    // Execute `aqueval bench` a second time
    let output2 = Command::new(bin_path)
        .arg("bench")
        .output()
        .expect("Failed to execute aqueval bench");

    assert!(output2.status.success());
    let stdout2 = String::from_utf8(output2.stdout).expect("Valid UTF-8");
    assert!(stdout2.contains(&format!("Nodes searched  : {SIGNATURE}")));
}
