use std::time::{Duration, Instant};
use quant::board::Board;
use quant::perft::positions::{
    ALL_POSITIONS, POS_3, POS_4, POS_5, POS_6, POS_KIWIPETE, POS_STARTPOS,
};
use quant::perft::{format_perft_divide, perft, perft_divide, perft_no_bulk};

#[test]
fn test_position_1_startpos_depths_1_to_6() {
    let mut board = Board::from_fen(POS_STARTPOS.fen).expect("valid startpos FEN");

    for depth in 1..=5 {
        let expected = POS_STARTPOS.expected_nodes(depth).unwrap();
        let nodes = perft(&mut board, depth);
        assert_eq!(
            nodes, expected,
            "Startpos depth {} failed: got {}, expected {}",
            depth, nodes, expected
        );
    }

    // Depth 6: Verify correctness and ensure it completes in <10 seconds in release mode.
    let start = Instant::now();
    let d6_nodes = perft(&mut board, 6);
    let elapsed = start.elapsed();
    let expected_d6 = POS_STARTPOS.expected_nodes(6).unwrap();

    assert_eq!(
        d6_nodes, expected_d6,
        "Startpos depth 6 failed: got {}, expected {}",
        d6_nodes, expected_d6
    );

    println!(
        "Startpos Depth 6: {} nodes in {:?} ({:.2} Mnps)",
        d6_nodes,
        elapsed,
        d6_nodes as f64 / (elapsed.as_secs_f64() * 1_000_000.0)
    );

    if !cfg!(debug_assertions) {
        assert!(
            elapsed < Duration::from_secs(10),
            "Hard requirement failed: Startpos D6 took {:?} (>= 10 seconds)",
            elapsed
        );
    }
}

#[test]
fn test_position_2_kiwipete_depths_1_to_5() {
    let mut board = Board::from_fen(POS_KIWIPETE.fen).expect("valid kiwipete FEN");

    for depth in 1..=5 {
        let expected = POS_KIWIPETE.expected_nodes(depth).unwrap();
        let nodes = perft(&mut board, depth);
        assert_eq!(
            nodes, expected,
            "Kiwipete depth {} failed: got {}, expected {}",
            depth, nodes, expected
        );
    }
}

#[test]
fn test_position_3_depths_1_to_6() {
    let mut board = Board::from_fen(POS_3.fen).expect("valid position 3 FEN");

    for depth in 1..=6 {
        let expected = POS_3.expected_nodes(depth).unwrap();
        let nodes = perft(&mut board, depth);
        assert_eq!(
            nodes, expected,
            "Position 3 depth {} failed: got {}, expected {}",
            depth, nodes, expected
        );
    }
}

#[test]
fn test_position_4_depths_1_to_6() {
    let mut board = Board::from_fen(POS_4.fen).expect("valid position 4 FEN");

    let max_depth = if cfg!(debug_assertions) { 5 } else { 6 };
    for depth in 1..=max_depth {
        let expected = POS_4.expected_nodes(depth).unwrap();
        let nodes = perft(&mut board, depth);
        assert_eq!(
            nodes, expected,
            "Position 4 depth {} failed: got {}, expected {}",
            depth, nodes, expected
        );
    }
}

#[test]
fn test_position_5_depths_1_to_5() {
    let mut board = Board::from_fen(POS_5.fen).expect("valid position 5 FEN");

    for depth in 1..=5 {
        let expected = POS_5.expected_nodes(depth).unwrap();
        let nodes = perft(&mut board, depth);
        assert_eq!(
            nodes, expected,
            "Position 5 depth {} failed: got {}, expected {}",
            depth, nodes, expected
        );
    }
}

#[test]
fn test_position_6_depths_1_to_5() {
    let mut board = Board::from_fen(POS_6.fen).expect("valid position 6 FEN");

    for depth in 1..=5 {
        let expected = POS_6.expected_nodes(depth).unwrap();
        let nodes = perft(&mut board, depth);
        assert_eq!(
            nodes, expected,
            "Position 6 depth {} failed: got {}, expected {}",
            depth, nodes, expected
        );
    }
}

#[test]
fn test_bulk_vs_non_bulk_consistency() {
    for pos in &ALL_POSITIONS {
        let mut board = Board::from_fen(pos.fen).expect("valid FEN");
        // Verify depths 1 to 3 across all positions (depth 3 on Kiwipete/Pos 6 is ~90k-100k nodes)
        for depth in 1..=3 {
            let bulk_count = perft(&mut board, depth);
            let non_bulk_count = perft_no_bulk(&mut board, depth);
            assert_eq!(
                bulk_count, non_bulk_count,
                "Position '{}' depth {}: bulk ({}) != non-bulk ({})",
                pos.name, depth, bulk_count, non_bulk_count
            );
        }
    }

    // Verify Startpos depth 4 explicitly
    let mut startpos = Board::startpos();
    assert_eq!(
        perft(&mut startpos, 4),
        perft_no_bulk(&mut startpos, 4),
        "Startpos depth 4 bulk must match non-bulk"
    );
}

#[test]
fn test_perft_divide_output_and_sum() {
    let mut board = Board::startpos();

    for depth in 1..=4 {
        let divide_res = perft_divide(&mut board, depth);
        let sum: u64 = divide_res.iter().map(|(_, n)| *n).sum();
        let expected_total = POS_STARTPOS.expected_nodes(depth).unwrap();

        assert_eq!(
            sum, expected_total,
            "Divide sum at depth {} must equal expected total",
            depth
        );

        // Verify divide results are sorted alphabetically
        for window in divide_res.windows(2) {
            let uci_a = window[0].0.to_uci();
            let uci_b = window[1].0.to_uci();
            assert!(
                uci_a < uci_b,
                "Divide moves must be sorted alphabetically: {} >= {}",
                uci_a,
                uci_b
            );
        }
    }

    // Verify formatted output for depth 1
    let divide_d1 = perft_divide(&mut board, 1);
    let formatted = format_perft_divide(&divide_d1);
    assert!(formatted.starts_with("a2a3: 1\n"));
    assert!(formatted.contains("h2h4: 1\n"));
    assert!(formatted.ends_with("\nNodes searched: 20\n"));
}

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static THREAD_TRACKING: Cell<bool> = const { Cell::new(false) };
    static THREAD_ALLOC_COUNT: Cell<usize> = const { Cell::new(0) };
}

struct TrackingAllocator;

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = THREAD_TRACKING.try_with(|tracking| {
            if tracking.get() {
                let _ = THREAD_ALLOC_COUNT.try_with(|c| c.set(c.get() + 1));
            }
        });
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let _ = THREAD_TRACKING.try_with(|tracking| {
            if tracking.get() {
                let _ = THREAD_ALLOC_COUNT.try_with(|c| c.set(c.get() + 1));
            }
        });
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = THREAD_TRACKING.try_with(|tracking| {
            if tracking.get() {
                let _ = THREAD_ALLOC_COUNT.try_with(|c| c.set(c.get() + 1));
            }
        });
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

#[test]
fn test_zero_allocations_in_perft_recursion() {
    let mut board = Board::startpos();

    // Warm up
    let _ = perft(&mut board, 1);

    THREAD_ALLOC_COUNT.with(|c| c.set(0));
    THREAD_TRACKING.with(|t| t.set(true));

    let nodes = perft(&mut board, 4);

    THREAD_TRACKING.with(|t| t.set(false));
    let allocs = THREAD_ALLOC_COUNT.with(|c| c.get());

    assert_eq!(nodes, 197_281);
    assert_eq!(
        allocs, 0,
        "Perft hot path recursion made {} heap allocations; expected exactly 0",
        allocs
    );
}
