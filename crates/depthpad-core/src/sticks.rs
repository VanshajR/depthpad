//! Opposing keys -> one stick axis.
//!
//! Port of HallJoy's `ResolveAxis` (configured_xusb_builder.cpp, AGPL-3.0, PashOK7/HallJoy):
//! * default: plus - minus;
//! * last key priority (SOCD "last input wins"): with both keys held the most recently pressed
//!   wins; a held key pushed deeper again by `sensitivity` retakes priority;
//! * snappy: the deeper key's full value in the winning direction instead of the difference.
//! Inputs are curved values 0..1; a key counts as pressed at 0.10.

const PRESSED: f64 = 0.10;
const EQUAL_EPSILON: f64 = 0.002;

#[derive(Debug, Clone)]
pub struct AxisResolver {
    snappy: bool,
    last_key_priority: bool,
    sensitivity: f64,
    prev_minus: bool,
    prev_plus: bool,
    last_direction: i8,
    minus_valley: f64,
    plus_valley: f64,
}

impl AxisResolver {
    pub fn new(snappy: bool, last_key_priority: bool, sensitivity: f64) -> Self {
        Self {
            snappy,
            last_key_priority,
            sensitivity: sensitivity.clamp(0.02, 0.95),
            prev_minus: false,
            prev_plus: false,
            last_direction: 0,
            minus_valley: 1.0,
            plus_valley: 1.0,
        }
    }

    pub fn resolve(&mut self, minus: f64, plus: f64) -> f64 {
        if !self.snappy && !self.last_key_priority {
            return plus - minus;
        }
        let (minus_down, plus_down) = (minus >= PRESSED, plus >= PRESSED);
        if minus_down && !self.prev_minus {
            self.last_direction = -1;
        }
        if plus_down && !self.prev_plus {
            self.last_direction = 1;
        }
        if self.last_key_priority {
            Self::valley(&mut self.minus_valley, minus, minus_down, self.prev_minus, self.sensitivity,
                         &mut self.last_direction, -1);
            Self::valley(&mut self.plus_valley, plus, plus_down, self.prev_plus, self.sensitivity,
                         &mut self.last_direction, 1);
        }
        self.prev_minus = minus_down;
        self.prev_plus = plus_down;

        let maximum = minus.max(plus);
        if maximum <= 0.0001 {
            return 0.0;
        }
        if self.last_key_priority {
            if minus_down && !plus_down {
                return -minus;
            }
            if plus_down && !minus_down {
                return plus;
            }
            if minus_down && plus_down {
                let direction = if self.last_direction != 0 { self.last_direction } else if plus >= minus { 1 } else { -1 };
                let magnitude = if self.snappy { maximum } else if direction > 0 { plus } else { minus };
                return if direction > 0 { magnitude } else { -magnitude };
            }
        }
        if self.snappy {
            let difference = plus - minus;
            if difference.abs() > EQUAL_EPSILON {
                return if difference > 0.0 { maximum } else { -maximum };
            }
            return match self.last_direction {
                d if d > 0 => maximum,
                d if d < 0 => -maximum,
                _ => 0.0,
            };
        }
        plus - minus
    }

    fn valley(valley: &mut f64, value: f64, down: bool, prev: bool, sensitivity: f64, dir: &mut i8, sign: i8) {
        if !down {
            *valley = 1.0;
        } else if !prev {
            *valley = value;
        } else {
            *valley = valley.min(value);
            if value - *valley >= sensitivity {
                *dir = sign;
                *valley = value;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_difference() {
        let mut r = AxisResolver::new(false, false, 0.02);
        assert_eq!(r.resolve(0.0, 0.7), 0.7);
        assert!((r.resolve(0.3, 0.7) - 0.4).abs() < 1e-9);
    }

    #[test]
    fn last_key_priority() {
        let mut r = AxisResolver::new(false, true, 0.02);
        assert_eq!(r.resolve(0.8, 0.0), -0.8);
        assert_eq!(r.resolve(0.8, 0.6), 0.6);
        assert_eq!(r.resolve(0.8, 0.0), -0.8);
        let mut r = AxisResolver::new(false, true, 0.02);
        r.resolve(1.0, 0.0);
        r.resolve(1.0, 1.0);
        r.resolve(0.5, 1.0);
        assert_eq!(r.resolve(0.6, 1.0), -0.6); // pushed 0.1 deeper: retakes priority
    }

    #[test]
    fn snappy() {
        let mut r = AxisResolver::new(true, false, 0.02);
        assert_eq!(r.resolve(0.3, 0.7), 0.7);
        let mut r = AxisResolver::new(true, false, 0.02);
        r.resolve(0.0, 0.5);
        assert_eq!(r.resolve(0.5, 0.5), -0.5); // equal -> the most recently pressed side
    }
}
