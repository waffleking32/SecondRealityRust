//! The fixed 70.086 Hz logic tick: the demo's heartbeat.

use std::time::Duration;

/// One logic tick is this many accumulator units; a nanosecond adds
/// `HZ_MILLI`, so ticks = ns × 70,086 / 1e12 and a period is 14,268,184.80 ns.
const TICK_SCALE: u64 = 1_000_000_000_000;

/// The logic clock: elapsed wall-clock time in, whole 70.086 Hz ticks out.
pub struct Tick {
    acc: u64,
    total: u64,
}

impl Default for Tick {
    fn default() -> Self {
        Tick::new()
    }
}

impl Tick {
    /// VGA vertical refresh, in ticks per second × 1000.
    pub const HZ_MILLI: u64 = 70_086;
    /// Elapsed wall-clock beyond this is dropped, not caught up.
    pub const MAX_ELAPSED: Duration = Duration::from_millis(250);

    /// Zeroed accumulator.
    pub fn new() -> Tick {
        Tick { acc: 0, total: 0 }
    }

    /// Feed elapsed wall-clock time; returns how many logic ticks are now due.
    /// Elapsed beyond `MAX_ELAPSED` is discarded.
    pub fn advance(&mut self, elapsed: Duration) -> u32 {
        // Clamping before the multiply also keeps the product inside u64.
        let elapsed = elapsed.min(Self::MAX_ELAPSED);
        self.acc += elapsed.as_nanos() as u64 * Self::HZ_MILLI;
        let mut due = 0u32;
        while self.acc >= TICK_SCALE {
            self.acc -= TICK_SCALE;
            self.total += 1;
            due += 1;
        }
        due
    }

    /// Ticks emitted since construction.
    pub fn total(&self) -> u64 {
        self.total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    /// One tick is 1e12 accumulator units at 70_086 units per nanosecond, so a
    /// period is 14,268,184.80 ns and the first whole tick lands at 14,268,185.
    /// Note this is the *unclamped* boundary; 250 ms is 17 ticks.
    #[rstest]
    #[case(0, 0)]
    #[case(14_268_184, 0)]
    #[case(14_268_185, 1)]
    #[case(250_000_000, 17)]
    fn advance_emits_whole_ticks_only(#[case] elapsed_ns: u64, #[case] ticks: u32) {
        let mut t = Tick::new();
        assert_eq!(t.advance(Duration::from_nanos(elapsed_ns)), ticks);
    }

    #[rstest]
    fn a_partial_tick_carries_into_the_next_call() {
        let mut t = Tick::new();
        assert_eq!(t.advance(Duration::from_nanos(14_268_184)), 0);
        assert_eq!(t.advance(Duration::from_nanos(1)), 1);
    }

    /// Exactness is measured in unclamped slices, because `advance` discards
    /// anything past `MAX_ELAPSED`. 4000 × 250 ms is exactly 1000 s.
    #[rstest]
    fn a_thousand_seconds_yields_exactly_70086_ticks() {
        let mut t = Tick::new();
        let mut ticks = 0u64;
        for _ in 0..4000 {
            ticks += t.advance(Tick::MAX_ELAPSED) as u64;
        }
        assert_eq!(ticks, 70_086);
        assert_eq!(t.total(), 70_086);
    }

    /// 250 ms is the whole budget; anything beyond it is dropped, not caught up.
    #[rstest]
    fn elapsed_beyond_max_is_clamped_to_max() {
        let mut a = Tick::new();
        let mut b = Tick::new();
        assert_eq!(a.advance(Tick::MAX_ELAPSED), 17);
        assert_eq!(
            b.advance(Tick::MAX_ELAPSED * 4),
            17,
            "excess time must be dropped, not caught up"
        );
        assert_eq!(a.total(), b.total());
    }
}
