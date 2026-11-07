use std::time::Duration;
use crate::types::Color;

/// Time limits computed for search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TimeLimits {
    pub soft_limit: Option<Duration>,
    pub hard_limit: Option<Duration>,
}

pub struct TimeManager;

impl TimeManager {
    /// Computes soft and hard search time limits given the clock state and parameters.
    ///
    /// Callers pass clock and movetime values with the UCI "Move Overhead" already subtracted.
    /// - `movetime`: Exact time dedicated to this move (soft and hard limits equal `movetime`).
    /// - `wtime` / `btime`: Remaining clock time in milliseconds.
    /// - `winc` / `binc`: Clock increment in milliseconds.
    /// - `movestogo`: Moves until next time control (defaults to 25 if sudden death).
    pub fn calculate_limits(
        side: Color,
        wtime: Option<u64>,
        btime: Option<u64>,
        winc: Option<u64>,
        binc: Option<u64>,
        movestogo: Option<u64>,
        movetime: Option<u64>,
    ) -> TimeLimits {
        if let Some(mt) = movetime {
            let dur = Duration::from_millis(mt.max(1));
            return TimeLimits {
                soft_limit: Some(dur),
                hard_limit: Some(dur),
            };
        }

        let time = match side {
            Color::White => wtime,
            Color::Black => btime,
        };

        if let Some(remaining) = time {
            let inc = match side {
                Color::White => winc.unwrap_or(0),
                Color::Black => binc.unwrap_or(0),
            };

            let remaining = remaining.max(1);
            let moves = movestogo.unwrap_or(25).clamp(1, 50);

            // Base allocation per move
            let base = (remaining / moves) + (inc * 3 / 4);

            // Safety cap: one move never takes more than half the remaining time,
            // whatever the increment or movestogo say (low clocks would otherwise spiral).
            let max_alloc = (remaining / 2).max(1);

            let soft_ms = (base * 8 / 10).min(max_alloc).max(1);
            let hard_ms = (base * 3).min(max_alloc).max(soft_ms);

            TimeLimits {
                soft_limit: Some(Duration::from_millis(soft_ms)),
                hard_limit: Some(Duration::from_millis(hard_ms)),
            }
        } else {
            TimeLimits {
                soft_limit: None,
                hard_limit: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_movetime_limits() {
        let limits = TimeManager::calculate_limits(
            Color::White,
            None,
            None,
            None,
            None,
            None,
            Some(1000),
        );
        assert_eq!(limits.soft_limit, Some(Duration::from_millis(1000)));
        assert_eq!(limits.hard_limit, Some(Duration::from_millis(1000)));
    }

    #[test]
    fn test_clock_limits() {
        let limits = TimeManager::calculate_limits(
            Color::White,
            Some(60000),
            Some(60000),
            Some(1000),
            Some(1000),
            Some(30),
            None,
        );
        assert!(limits.soft_limit.is_some());
        assert!(limits.hard_limit.is_some());
        assert!(limits.soft_limit.unwrap() <= limits.hard_limit.unwrap());
    }

    #[test]
    fn test_low_clock_caps_at_half() {
        // 8 s left with a 3 s increment: base is about 3.4 s, so 3·base would take 10 s
        let limits = TimeManager::calculate_limits(Color::White, Some(8000), Some(8000), Some(3000), Some(3000), None, None);
        assert_eq!(limits.hard_limit, Some(Duration::from_millis(4000)));
        assert!(limits.soft_limit.unwrap() <= limits.hard_limit.unwrap());
    }

    #[test]
    fn test_no_time_limits() {
        let limits = TimeManager::calculate_limits(
            Color::White,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(limits.soft_limit, None);
        assert_eq!(limits.hard_limit, None);
    }

    #[test]
    fn test_hard_limit_never_exceeds_remaining() {
        for remaining in [1u64, 20, 100, 500, 1600, 60000] {
            for (inc, mtg) in [(0, None), (5000, None), (0, Some(1)), (10_000, Some(1))] {
                let l = TimeManager::calculate_limits(
                    Color::Black,
                    None,
                    Some(remaining),
                    None,
                    Some(inc),
                    mtg,
                    None,
                );
                let hard = l.hard_limit.unwrap().as_millis() as u64;
                assert!(hard <= (remaining * 8 / 10).max(1), "remaining {remaining} inc {inc}: hard {hard}");
                assert!(l.soft_limit.unwrap() <= l.hard_limit.unwrap());
            }
        }
    }
}
