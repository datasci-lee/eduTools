//! 레이저 꼬리의 시간 기반 순수 상태.

use crate::ink::Point;

/// Swift: LaserRenderer trail point `(location:time:)`
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaserPoint {
    pub location: Point,
    pub time: f64,
}

/// Swift: LaserRenderer.trails current trail
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaserTrail {
    pub trail: Vec<LaserPoint>,
}

impl LaserTrail {
    /// Swift: LaserRenderer.move(to:leavesTrail:)
    pub fn push(&mut self, point: LaserPoint) {
        self.trail.push(point);
    }

    /// Swift: LaserRenderer.retract()
    pub fn retract(&mut self, now: f64, duration: f64) {
        let cutoff = now - duration;
        if let Some(first_fresh) = self.trail.iter().position(|point| point.time >= cutoff) {
            if first_fresh > 1 {
                self.trail.drain(0..first_fresh - 1);
            }
        } else if self.trail.last().is_some_and(|point| point.time < cutoff) {
            self.trail.clear();
        }
    }
}
