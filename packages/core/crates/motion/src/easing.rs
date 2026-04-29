/// Cubic bezier easing for CSS-style timing functions.
///
/// Defined by two control points of a cubic bezier curve from (0,0) to (1,1).
/// Implements the same algorithm as https://github.com/gre/bezier-easing
/// which matches the CSS `transition-timing-function: cubic-bezier(...)` spec.
#[derive(Debug, Clone, Copy)]
pub struct Easing {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    /// Pre-computed sample table for fast X→t lookup.
    sample_table: [f64; SAMPLE_TABLE_SIZE],
}

const SAMPLE_TABLE_SIZE: usize = 11;
const SAMPLE_STEP_SIZE: f64 = 1.0 / (SAMPLE_TABLE_SIZE as f64 - 1.0);
const NEWTON_ITERATIONS: usize = 4;
const NEWTON_MIN_SLOPE: f64 = 0.001;
const SUBDIVISION_PRECISION: f64 = 0.0000001;
const SUBDIVISION_MAX_ITERATIONS: usize = 10;

// Cubic bezier basis functions for one axis:
//   B(t) = A*t^3 + B*t^2 + C*t
// where A = 1 - 3*p2 + 3*p1, B = 3*p2 - 6*p1, C = 3*p1
#[inline]
fn coeff_a(p1: f64, p2: f64) -> f64 {
    1.0 - 3.0 * p2 + 3.0 * p1
}
#[inline]
fn coeff_b(p1: f64, p2: f64) -> f64 {
    3.0 * p2 - 6.0 * p1
}
#[inline]
fn coeff_c(p1: f64) -> f64 {
    3.0 * p1
}

/// Evaluate the cubic bezier at parameter t for one axis.
#[inline]
fn bezier_at(t: f64, p1: f64, p2: f64) -> f64 {
    ((coeff_a(p1, p2) * t + coeff_b(p1, p2)) * t + coeff_c(p1)) * t
}

/// Derivative of the cubic bezier at parameter t for one axis.
#[inline]
fn bezier_slope(t: f64, p1: f64, p2: f64) -> f64 {
    3.0 * coeff_a(p1, p2) * t * t + 2.0 * coeff_b(p1, p2) * t + coeff_c(p1)
}

fn newton_raphson(x: f64, mut guess: f64, x1: f64, x2: f64) -> f64 {
    for _ in 0..NEWTON_ITERATIONS {
        let slope = bezier_slope(guess, x1, x2);
        if slope == 0.0 {
            break;
        }
        let current_x = bezier_at(guess, x1, x2) - x;
        guess -= current_x / slope;
    }
    guess
}

fn binary_subdivide(x: f64, mut a: f64, mut b: f64, x1: f64, x2: f64) -> f64 {
    let mut current_t;
    let mut current_x;
    for _ in 0..SUBDIVISION_MAX_ITERATIONS {
        current_t = a + (b - a) / 2.0;
        current_x = bezier_at(current_t, x1, x2) - x;
        if current_x.abs() <= SUBDIVISION_PRECISION {
            return current_t;
        }
        if current_x > 0.0 {
            b = current_t;
        } else {
            a = current_t;
        }
    }
    a + (b - a) / 2.0
}

fn build_sample_table(x1: f64, x2: f64) -> [f64; SAMPLE_TABLE_SIZE] {
    let mut table = [0.0; SAMPLE_TABLE_SIZE];
    for i in 0..SAMPLE_TABLE_SIZE {
        table[i] = bezier_at(i as f64 * SAMPLE_STEP_SIZE, x1, x2);
    }
    table
}

impl Easing {
    pub const LINEAR: Self = Self::cubic(0.0, 0.0, 1.0, 1.0);

    pub const EASE: Self = Self::cubic(0.25, 0.1, 0.25, 1.0);
    pub const EASE_IN: Self = Self::cubic(0.42, 0.0, 1.0, 1.0);
    pub const EASE_OUT: Self = Self::cubic(0.0, 0.0, 0.58, 1.0);
    pub const EASE_IN_OUT: Self = Self::cubic(0.42, 0.0, 0.58, 1.0);

    // Framer Motion named easings
    pub const CIRC_IN: Self = Self::cubic(0.55, 0.0, 1.0, 0.45);
    pub const CIRC_OUT: Self = Self::cubic(0.0, 0.55, 0.45, 1.0);
    pub const CIRC_IN_OUT: Self = Self::cubic(0.85, 0.0, 0.15, 1.0);
    pub const BACK_IN: Self = Self::cubic(0.36, 0.0, 0.66, -0.56);
    pub const BACK_OUT: Self = Self::cubic(0.34, 1.56, 0.64, 1.0);
    pub const BACK_IN_OUT: Self = Self::cubic(0.68, -0.6, 0.32, 1.6);
    pub const ANTICIPATE: Self = Self::cubic(0.36, 0.0, 0.66, -0.56);

    pub const fn cubic(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self {
            x1,
            y1,
            x2,
            y2,
            // Cannot call build_sample_table in const; filled on first use via apply().
            // For const instances we use a zeroed table and rebuild lazily.
            sample_table: [0.0; SAMPLE_TABLE_SIZE],
        }
    }

    /// Build sample table (non-const path, used at runtime).
    pub fn with_table(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self {
            x1,
            y1,
            x2,
            y2,
            sample_table: build_sample_table(x1, x2),
        }
    }

    /// Given time x ∈ [0,1], find the bezier parameter t such that bezier_x(t) = x,
    /// then return bezier_y(t). This is the correct CSS cubic-bezier semantics.
    pub fn apply(&self, x: f64) -> f64 {
        // Linear shortcut
        if self.x1 == self.y1 && self.x2 == self.y2 {
            return x;
        }
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }

        // Ensure table is built (handles const-constructed instances)
        let table = if self.sample_table[SAMPLE_TABLE_SIZE - 1] == 0.0
            && (self.x1 != 0.0 || self.x2 != 0.0)
        {
            build_sample_table(self.x1, self.x2)
        } else {
            self.sample_table
        };

        // Table lookup to find the interval containing x
        let mut interval_start = 0.0_f64;
        let mut current_sample = 1usize;
        let last_sample = SAMPLE_TABLE_SIZE - 1;

        while current_sample < last_sample && table[current_sample] <= x {
            interval_start += SAMPLE_STEP_SIZE;
            current_sample += 1;
        }
        current_sample -= 1;

        // Interpolate to get initial guess for t
        let dist = (x - table[current_sample])
            / (table[current_sample + 1] - table[current_sample]);
        let guess_for_t = interval_start + dist * SAMPLE_STEP_SIZE;

        // Refine t using Newton-Raphson or binary subdivision
        let t = {
            let initial_slope = bezier_slope(guess_for_t, self.x1, self.x2);
            if initial_slope >= NEWTON_MIN_SLOPE {
                newton_raphson(x, guess_for_t, self.x1, self.x2)
            } else if initial_slope == 0.0 {
                guess_for_t
            } else {
                binary_subdivide(
                    x,
                    interval_start,
                    interval_start + SAMPLE_STEP_SIZE,
                    self.x1,
                    self.x2,
                )
            }
        };

        // Evaluate Y at the found parameter
        bezier_at(t, self.y1, self.y2)
    }
}

/// Tween a single f64 from `a` to `b` at progress `t` with the given easing.
pub fn tween_f64(a: f64, b: f64, t: f64, easing: &Easing) -> f64 {
    let eased = easing.apply(t);
    a + (b - a) * eased
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_easing_identity() {
        let e = Easing::LINEAR;
        assert!((e.apply(0.0) - 0.0).abs() < 1e-6);
        assert!((e.apply(0.5) - 0.5).abs() < 1e-6);
        assert!((e.apply(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn tween_f64_endpoints() {
        assert!((tween_f64(10.0, 20.0, 0.0, &Easing::LINEAR) - 10.0).abs() < 1e-6);
        assert!((tween_f64(10.0, 20.0, 1.0, &Easing::LINEAR) - 20.0).abs() < 1e-6);
    }

    #[test]
    fn ease_in_out_midpoint() {
        // ease-in-out should be approximately 0.5 at t=0.5 (symmetric curve)
        let e = Easing::with_table(0.42, 0.0, 0.58, 1.0);
        let mid = e.apply(0.5);
        assert!((mid - 0.5).abs() < 0.05, "ease-in-out midpoint was {mid}");
    }

    #[test]
    fn back_in_out_overshoots() {
        // cubic-bezier(0.68, -0.6, 0.32, 1.6) should overshoot below 0 near start
        // and above 1 near end
        let e = Easing::with_table(0.68, -0.6, 0.32, 1.6);
        let early = e.apply(0.1);
        assert!(early < 0.0, "back-in-out at t=0.1 should undershoot, got {early}");
        let late = e.apply(0.9);
        assert!(late > 1.0, "back-in-out at t=0.9 should overshoot, got {late}");
        // endpoints still correct
        assert!((e.apply(0.0)).abs() < 1e-6);
        assert!((e.apply(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ease_monotonically_increasing_for_standard_curves() {
        let e = Easing::with_table(0.25, 0.1, 0.25, 1.0);
        let mut prev = 0.0;
        for i in 1..=100 {
            let t = i as f64 / 100.0;
            let y = e.apply(t);
            assert!(y >= prev - 1e-10, "ease should be monotonic at t={t}: {prev} -> {y}");
            prev = y;
        }
    }
}
