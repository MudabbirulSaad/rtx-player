use std::time::{Duration, Instant};

const RATES: [f64; 7] = [1.0, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0];

#[derive(Default)]
pub struct Adaptive {
    pub enabled: bool,
    pub requested: f64,
    pub reason: Option<String>,
    previous: Option<(Instant, f64, f64)>,
    overloaded: Duration,
    last_change: Option<Instant>,
    memory_since: Option<Instant>,
}

impl Adaptive {
    pub fn reset(&mut self, requested: f64) {
        self.requested = requested;
        self.previous = None;
        self.overloaded = Duration::ZERO;
        self.last_change = None;
        self.memory_since = None;
        self.reason = None;
    }
    pub fn memory_budget(
        &mut self,
        now: Instant,
        playing: bool,
        free_mib: Option<u64>,
        rate: f64,
    ) -> Option<f64> {
        if !self.enabled || !playing || rate <= 1. || free_mib.is_none_or(|free| free >= 512) {
            self.memory_since = None;
            return None;
        }
        if now.duration_since(*self.memory_since.get_or_insert(now)) < Duration::from_secs(3)
            || self
                .last_change
                .is_some_and(|t| now.duration_since(t) < Duration::from_secs(8))
        {
            return None;
        }
        self.last_change = Some(now);
        self.memory_since = None;
        self.reason = Some(format!(
            "RIFE {rate}× → original FPS after three seconds below 512 MiB of free GPU memory."
        ));
        Some(1.)
    }
    pub fn observe(
        &mut self,
        now: Instant,
        playing: bool,
        position: Option<f64>,
        drops: Option<f64>,
        avsync: Option<f64>,
        rate: f64,
    ) -> Option<f64> {
        if !self.enabled || !playing || rate <= 1.0 {
            self.previous = None;
            self.overloaded = Duration::ZERO;
            return None;
        }
        let (position, drops) = (position?, drops?);
        let Some((then, last_pos, last_drops)) = self.previous else {
            self.previous = Some((now, position, drops));
            return None;
        };
        let dt = now.duration_since(then);
        if dt < Duration::from_secs(1) {
            return None;
        }
        self.previous = Some((now, position, drops));
        if position < last_pos
            || position - last_pos > dt.as_secs_f64() * 4.0 + 1.
            || drops < last_drops
            || dt > Duration::from_secs(4)
        {
            self.overloaded = Duration::ZERO;
            return None;
        }
        let drift = avsync.unwrap_or(0.).abs();
        let loss = (drops - last_drops) / dt.as_secs_f64();
        if loss >= 2.0 || drift > 0.15 {
            self.overloaded += dt
        } else {
            self.overloaded = Duration::ZERO
        }
        if self.overloaded < Duration::from_secs(3)
            || self
                .last_change
                .is_some_and(|t| now.duration_since(t) < Duration::from_secs(8))
        {
            return None;
        }
        let next = if drift > 0.5 {
            1.0
        } else {
            RATES.iter().rev().copied().find(|v| *v < rate)?
        };
        self.reason = Some(format!(
            "RIFE {rate}× → {next}× after {loss:.1} drops/s and {:.0} ms A/V offset.",
            drift * 1000.
        ));
        self.overloaded = Duration::ZERO;
        self.last_change = Some(now);
        self.previous = None;
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handles_sustained_memory_pressure_on_the_active_adapter() {
        let now = Instant::now();
        let mut policy = Adaptive {
            enabled: true,
            ..Default::default()
        };
        for sec in 0..3 {
            assert_eq!(
                policy.memory_budget(now + Duration::from_secs(sec), true, Some(320), 4.),
                None
            );
        }
        assert_eq!(
            policy.memory_budget(now + Duration::from_secs(3), true, Some(320), 4.),
            Some(1.)
        );
        assert!(policy.reason.as_ref().unwrap().contains("memory"));
        assert_eq!(
            policy.memory_budget(now + Duration::from_secs(4), true, None, 4.),
            None
        );
    }
    #[test]
    fn ignores_transients_and_steps_down_after_sustained_misses() {
        let start = Instant::now();
        let mut a = Adaptive {
            enabled: true,
            ..Default::default()
        };
        a.reset(5.);
        let sample = |a: &mut Adaptive, sec: u64, drops: f64| {
            a.observe(
                start + Duration::from_secs(sec),
                true,
                Some(sec as f64),
                Some(drops),
                Some(0.),
                5.,
            )
        };
        assert_eq!(sample(&mut a, 0, 0.), None);
        assert_eq!(sample(&mut a, 1, 4.), None);
        assert_eq!(sample(&mut a, 2, 4.), None);
        assert_eq!(sample(&mut a, 3, 7.), None);
        assert_eq!(sample(&mut a, 4, 10.), None);
        assert_eq!(sample(&mut a, 5, 13.), Some(4.));
        assert_eq!(a.requested, 5.);
        assert!(a.reason.as_deref().unwrap().contains("3.0 drops/s"));
        for t in 6..12 {
            assert_eq!(sample(&mut a, t, 4. * t as f64), None)
        }
    }
}
