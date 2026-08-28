//! Continuous Gaussian Phi Accrual Failure Detector.

use std::collections::VecDeque;

const DEFAULT_WINDOW_SIZE: usize = 100;
const MIN_STD_DEV_MICROS: f64 = 50_000.0; // 50ms min std dev

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PeerHealthState {
    Alive,
    Suspect,
    Dead,
}

#[derive(Debug, Clone)]
pub struct PhiAccrualDetector {
    window_size: usize,
    intervals: VecDeque<f64>,
    last_heartbeat_micros: Option<u64>,
    phi_suspect: f64,
    phi_dead: f64,
}

impl Default for PhiAccrualDetector {
    fn default() -> Self {
        Self::new(8.0, 14.0)
    }
}

impl PhiAccrualDetector {
    #[must_use]
    pub fn new(phi_suspect: f64, phi_dead: f64) -> Self {
        Self {
            window_size: DEFAULT_WINDOW_SIZE,
            intervals: VecDeque::with_capacity(DEFAULT_WINDOW_SIZE),
            last_heartbeat_micros: None,
            phi_suspect,
            phi_dead,
        }
    }

    pub fn record_heartbeat(&mut self, now_micros: u64) {
        if let Some(prev) = self.last_heartbeat_micros
            && now_micros > prev
        {
            let interval = (now_micros - prev) as f64;
            if self.intervals.len() >= self.window_size {
                self.intervals.pop_front();
            }
            self.intervals.push_back(interval);
        }
        self.last_heartbeat_micros = Some(now_micros);
    }

    #[must_use]
    pub fn last_heartbeat_micros(&self) -> Option<u64> {
        self.last_heartbeat_micros
    }

    #[must_use]
    pub fn phi(&self, now_micros: u64) -> f64 {
        let last = match self.last_heartbeat_micros {
            Some(l) => l,
            None => return 0.0,
        };
        if now_micros <= last || self.intervals.len() < 2 {
            return 0.0;
        }
        let elapsed = (now_micros - last) as f64;
        let count = self.intervals.len() as f64;
        let mean = self.intervals.iter().sum::<f64>() / count;
        let variance = self
            .intervals
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>()
            / count;
        let std_dev = variance.sqrt().max(MIN_STD_DEV_MICROS);

        let y = (elapsed - mean) / (std_dev * std::f64::consts::SQRT_2);
        let p_later = 0.5 * erfc(y);
        if p_later <= 1e-15 {
            15.0
        } else {
            -p_later.log10()
        }
    }

    #[must_use]
    pub fn health(&self, now_micros: u64) -> PeerHealthState {
        let score = self.phi(now_micros);
        if score >= self.phi_dead {
            PeerHealthState::Dead
        } else if score >= self.phi_suspect {
            PeerHealthState::Suspect
        } else {
            PeerHealthState::Alive
        }
    }
}

/// Complementary Error Function approximation (Abramowitz & Stegun 7.1.26).
#[must_use]
pub fn erfc(x: f64) -> f64 {
    if x < 0.0 {
        return 2.0 - erfc(-x);
    }
    let p = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;

    let t = 1.0 / (1.0 + p * x);
    let poly = t * (a1 + t * (a2 + t * (a3 + t * (a4 + t * a5))));
    poly * (-x * x).exp()
}
