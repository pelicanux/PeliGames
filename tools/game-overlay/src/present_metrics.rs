//! Count Vulkan presents for every overlay session, optionally on both sides of a generator.
//! Sampling adapts MangoHUD's count / monotonic elapsed-time method to two
//! streams sharing a window. See NOTICES.md for the upstream MIT notice.
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Snapshot {
    pub game: f64,
    // Absent until the downstream observer actually contributes to the sample.
    pub output: Option<Output>,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Output {
    pub generated: f64,
    pub total: f64,
}
const SAMPLE_PERIOD: Duration = Duration::from_millis(500);
const STALE_AFTER: Duration = Duration::from_secs(3);

struct Meter {
    start: Instant,
    last: Instant,
    last_event: Instant,
    game: u64,
    total: u64,
    seen_game: bool,
    seen_total: bool,
    snapshot: Option<Snapshot>,
}
impl Meter {
    fn new(now: Instant) -> Self {
        Self {
            start: now,
            last: now,
            last_event: now,
            game: 0,
            total: 0,
            seen_game: false,
            seen_total: false,
            snapshot: None,
        }
    }
    fn add(&mut self, game: bool, now: Instant) {
        // The clock is read under the shared lock in record(). A resumed game
        // starts a fresh window rather than averaging an idle gap into its FPS.
        if now.saturating_duration_since(self.last_event) >= STALE_AFTER {
            *self = Self::new(now);
        }
        self.last_event = now;
        let elapsed = now.saturating_duration_since(self.start);
        if elapsed >= SAMPLE_PERIOD {
            if self.seen_game {
                let seconds = elapsed.as_secs_f64();
                let game = self.game as f64 / seconds;
                let total = self.total as f64 / seconds;
                self.snapshot = Some(Snapshot {
                    game,
                    output: self.seen_total.then_some(Output {
                        generated: (total - game).max(0.0),
                        total,
                    }),
                });
                self.last = now;
            }
            self.start = now;
            self.game = 0;
            self.total = 0;
            self.seen_game = false;
            self.seen_total = false;
        }
        // Like MangoHUD, close the previous sample BEFORE counting this frame.
        // Both streams then use [start, end), avoiding an extra frame at the
        // boundary and a biased first sample. No assumed frame multiplier.
        if game {
            self.game += 1;
            self.seen_game = true;
        } else {
            self.total += 1;
            self.seen_total = true;
        }
    }
    fn snapshot(&self, now: Instant) -> Option<Snapshot> {
        (now.saturating_duration_since(self.last) < STALE_AFTER)
            .then_some(self.snapshot)
            .flatten()
    }
}
static METERS: OnceLock<Mutex<HashMap<u64, Meter>>> = OnceLock::new();
static PRIMARY: AtomicU64 = AtomicU64::new(0);
pub(crate) fn generator() -> Option<crate::present_routing::Generator> {
    static GENERATOR: OnceLock<Option<crate::present_routing::Generator>> = OnceLock::new();
    *GENERATOR.get_or_init(|| {
        crate::present_routing::detect(
            std::env::var("AFMF_ENABLE").ok().as_deref() == Some("1"),
            &std::env::var("VK_INSTANCE_LAYERS").unwrap_or_default(),
        )
    })
}
pub(crate) fn renderer_is_source() -> bool {
    !generator().is_some_and(|generator| generator.counter_is_source())
}
pub(crate) fn record(id: u64, game: bool) {
    if let Ok(mut meters) = METERS.get_or_init(Default::default).lock() {
        // Real and generated presents may arrive on different threads. Reading
        // the clock after acquiring the lock preserves their accounting order.
        let now = Instant::now();
        // Bound bookkeeping even when an application creates many swapchains.
        if meters.len() >= 16 && !meters.contains_key(&id) {
            return;
        }
        meters
            .entry(id)
            .or_insert_with(|| Meter::new(now))
            .add(game, now);
    }
    if game {
        PRIMARY.store(id, Ordering::Relaxed);
    }
}
pub(crate) fn forget(id: u64) {
    if let Some(meters) = METERS.get() {
        if let Ok(mut m) = meters.lock() {
            m.remove(&id);
        }
    }
    let _ = PRIMARY.compare_exchange(id, 0, Ordering::Relaxed, Ordering::Relaxed);
}
pub(crate) fn snapshot() -> Option<Snapshot> {
    let id = PRIMARY.load(Ordering::Relaxed);
    METERS
        .get()?
        .lock()
        .ok()?
        .get(&id)?
        .snapshot(Instant::now())
}
#[cfg(test)]
mod tests {
    use super::*;

    // Deterministic present traces: no game, GPU, environment or sleeps.
    fn trace(
        meter: &mut Meter,
        start: Instant,
        ticks: u64,
        tick_hz: u64,
        game_every: u64,
        total_every: u64,
    ) {
        for tick in 0..=ticks {
            let now = start + Duration::from_nanos(tick * 1_000_000_000 / tick_hz);
            if tick % game_every == 0 {
                meter.add(true, now);
            }
            if tick % total_every == 0 {
                meter.add(false, now);
            }
        }
    }
    fn assert_fps(s: Snapshot, game: f64, total: f64) {
        assert!((s.game - game).abs() < 0.01, "game: {s:?}");
        let output = s.output.expect("downstream presents were observed");
        assert!((output.total - total).abs() < 0.01, "total: {s:?}");
        assert!(
            (output.generated - (total - game).max(0.0)).abs() < 0.01,
            "generated: {s:?}"
        );
    }
    #[test]
    fn paced_present_traces_count_actual_generation_instead_of_assuming_two_times() {
        for (tick_hz, game_every, total_every, game, total) in [
            (120, 2, 1, 60.0, 120.0),
            (300, 5, 3, 60.0, 100.0),
            (60, 1, 1, 60.0, 60.0),
            (360, 3, 1, 120.0, 360.0),
        ] {
            let start = Instant::now();
            let mut meter = Meter::new(start);
            trace(
                &mut meter,
                start,
                2 * tick_hz,
                tick_hz,
                game_every,
                total_every,
            );
            assert_fps(
                meter.snapshot(start + Duration::from_secs(2)).unwrap(),
                game,
                total,
            );
        }
    }
    #[test]
    fn first_half_second_sample_does_not_include_the_next_windows_frame() {
        let start = Instant::now();
        let mut meter = Meter::new(start);
        trace(&mut meter, start, 59, 120, 2, 1);
        assert!(meter.snapshot(start + Duration::from_millis(499)).is_none());
        meter.add(true, start + SAMPLE_PERIOD);
        assert_fps(meter.snapshot(start + SAMPLE_PERIOD).unwrap(), 60.0, 120.0);
        assert_eq!((meter.game, meter.total), (1, 0));
    }
    #[test]
    fn count_uses_elapsed_time_with_irregular_pacing_not_mean_instantaneous_fps() {
        let start = Instant::now();
        let mut meter = Meter::new(start);
        // Two separated bursts, ten accepted presents across one half second.
        for ms in [0, 1, 2, 3, 4, 300, 350, 400, 450, 499] {
            let now = start + Duration::from_millis(ms);
            meter.add(true, now);
            meter.add(false, now);
        }
        meter.add(true, start + SAMPLE_PERIOD);
        assert_fps(meter.snapshot(start + SAMPLE_PERIOD).unwrap(), 20.0, 20.0);
    }
    #[test]
    fn reversing_stream_order_at_the_same_timestamp_does_not_change_the_rate() {
        let start = Instant::now();
        let mut meter = Meter::new(start);
        for tick in 0..=240 {
            let now = start + Duration::from_nanos(tick * 1_000_000_000 / 120);
            meter.add(false, now);
            if tick % 2 == 0 {
                meter.add(true, now);
            }
        }
        assert_fps(
            meter.snapshot(start + Duration::from_secs(2)).unwrap(),
            60.0,
            120.0,
        );
    }
    #[test]
    fn generation_change_replaces_the_previous_rate_at_the_next_sample() {
        let start = Instant::now();
        let mut meter = Meter::new(start);
        trace(&mut meter, start, 59, 120, 2, 1);
        // First event closes the 60 / 120 window, then opens a 60 / 60 window.
        let second = start + SAMPLE_PERIOD;
        trace(&mut meter, second, 30, 60, 1, 1);
        assert_fps(
            meter.snapshot(start + Duration::from_secs(1)).unwrap(),
            60.0,
            60.0,
        );
    }
    #[test]
    fn mako_fixed_adaptive_and_paused_frames_use_observed_output_counts() {
        let start = Instant::now();
        let mut meter = Meter::new(start);
        // MAKO renderer observes output before the passive source observer
        // returns to the game. Exercise fixed3x, fractional output, then pause.
        for (window, total_hz) in [(0, 180), (1, 100), (2, 60)] {
            let window_start = start + Duration::from_secs(window);
            for tick in 0..900 {
                let now = window_start + Duration::from_nanos(tick * 1_000_000_000 / 900);
                if tick % (900 / total_hz) == 0 {
                    meter.add(false, now);
                }
                if tick % 15 == 0 {
                    meter.add(true, now);
                }
            }
            let end = window_start + Duration::from_secs(1);
            meter.add(false, end);
            assert_fps(meter.snapshot(end).unwrap(), 60., total_hz as f64);
            // End event belongs to the next window and must not be doubled.
            meter = Meter::new(end);
        }
    }

    #[test]
    fn basic_fps_works_without_a_generator_or_downstream_observer() {
        let start = Instant::now();
        let mut meter = Meter::new(start);
        for tick in 0..=120 {
            meter.add(
                true,
                start + Duration::from_nanos(tick * 1_000_000_000 / 60),
            );
        }
        let sample = meter.snapshot(start + Duration::from_secs(2)).unwrap();
        assert!((sample.game - 60.).abs() < 0.01);
        assert!(sample.output.is_none());
    }
    #[test]
    fn downstream_observer_alone_cannot_invent_the_games_fps() {
        let start = Instant::now();
        let mut meter = Meter::new(start);
        for tick in 0..=120 {
            meter.add(
                false,
                start + Duration::from_nanos(tick * 1_000_000_000 / 120),
            );
        }
        assert!(meter.snapshot(start + Duration::from_secs(1)).is_none());
    }
    #[test]
    fn losing_downstream_measurement_keeps_basic_fps_without_stale_generation() {
        let start = Instant::now();
        let mut meter = Meter::new(start);
        trace(&mut meter, start, 59, 120, 2, 1);
        meter.add(true, start + SAMPLE_PERIOD);
        assert!(meter
            .snapshot(start + SAMPLE_PERIOD)
            .unwrap()
            .output
            .is_some());
        for tick in 31..=90 {
            meter.add(
                true,
                start + Duration::from_nanos(tick * 1_000_000_000 / 60),
            );
        }
        let sample = meter.snapshot(start + Duration::from_millis(1500)).unwrap();
        assert!((sample.game - 60.).abs() < 0.01);
        assert!(sample.output.is_none());
        assert!(meter.snapshot(start + Duration::from_secs(5)).is_none());
    }
    #[test]
    fn pause_expires_snapshot_and_resumes_without_including_the_idle_gap() {
        let start = Instant::now();
        let mut meter = Meter::new(start);
        trace(&mut meter, start, 60, 120, 2, 1);
        let resumed = start + Duration::from_secs(10);
        assert!(meter.snapshot(resumed).is_none());
        meter.add(true, resumed);
        assert!(meter.snapshot(resumed).is_none());
        meter.add(false, resumed);
        // Continue past the already-counted pair at t=0 of this new window.
        for tick in 1..=60 {
            let now = resumed + Duration::from_nanos(tick * 1_000_000_000 / 120);
            if tick % 2 == 0 {
                meter.add(true, now);
            }
            meter.add(false, now);
        }
        assert_fps(
            meter.snapshot(resumed + SAMPLE_PERIOD).unwrap(),
            60.0,
            120.0,
        );
    }
}

#[cfg(test)]
pub(crate) fn pending_counts(id: u64) -> Option<(u64, u64)> {
    METERS
        .get()?
        .lock()
        .ok()?
        .get(&id)
        .map(|m| (m.game, m.total))
}
