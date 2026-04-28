/// Damped harmonic oscillator spring solver.
///
/// Supports underdamped (ζ < 1), critically damped (ζ = 1), and overdamped (ζ > 1)
/// regimes. Parameter names match Framer Motion conventions.
#[derive(Debug, Clone, Copy)]
pub struct SpringParams {
    pub stiffness: f64,
    pub damping: f64,
    pub mass: f64,
    pub initial_velocity: f64,
    pub rest_delta: f64,
    pub rest_speed: f64,
}

impl Default for SpringParams {
    fn default() -> Self {
        Self {
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
            initial_velocity: 0.0,
            rest_delta: 0.01,
            rest_speed: 0.01,
        }
    }
}

/// Result of sampling a spring at a given time.
#[derive(Debug, Clone, Copy)]
pub struct SpringSample {
    pub value: f64,
    pub velocity: f64,
    pub settled: bool,
}

/// Solve a spring from `origin` to `target` at elapsed time `t` seconds.
///
/// The spring models `x'' + (c/m)x' + (k/m)x = 0` with the equilibrium
/// shifted to `target`. Returns displacement from target and velocity.
pub fn solve_spring(params: &SpringParams, origin: f64, target: f64, t: f64) -> SpringSample {
    let displacement = origin - target;
    let v0 = params.initial_velocity;

    let omega_n = (params.stiffness / params.mass).sqrt();
    let zeta = params.damping / (2.0 * (params.stiffness * params.mass).sqrt());

    let (x, v) = if (zeta - 1.0).abs() < 1e-6 {
        // Critically damped: x(t) = (A + Bt) * e^(-ωt)
        let a = displacement;
        let b = v0 + omega_n * displacement;
        let exp = (-omega_n * t).exp();
        let x = (a + b * t) * exp;
        let v = (b - omega_n * (a + b * t)) * exp;
        (x, v)
    } else if zeta < 1.0 {
        // Underdamped: x(t) = e^(-ζωt) * (A cos(ωd t) + B sin(ωd t))
        let omega_d = omega_n * (1.0 - zeta * zeta).sqrt();
        let a = displacement;
        let b = (v0 + zeta * omega_n * displacement) / omega_d;
        let exp = (-zeta * omega_n * t).exp();
        let cos = (omega_d * t).cos();
        let sin = (omega_d * t).sin();
        let x = exp * (a * cos + b * sin);
        let v = exp
            * ((b * omega_d - a * zeta * omega_n) * cos - (a * omega_d + b * zeta * omega_n) * sin);
        (x, v)
    } else {
        // Overdamped: x(t) = A e^(r1 t) + B e^(r2 t)
        let s = (zeta * zeta - 1.0).sqrt();
        let r1 = -omega_n * (zeta + s);
        let r2 = -omega_n * (zeta - s);
        let b = (r1 * displacement - v0) / (r1 - r2);
        let a = displacement - b;
        let e1 = (r1 * t).exp();
        let e2 = (r2 * t).exp();
        let x = a * e1 + b * e2;
        let v = a * r1 * e1 + b * r2 * e2;
        (x, v)
    };

    let settled = x.abs() < params.rest_delta && v.abs() < params.rest_speed;

    SpringSample {
        value: target + x,
        velocity: v,
        settled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critically_damped_converges() {
        let params = SpringParams {
            stiffness: 100.0,
            damping: 2.0 * (100.0_f64).sqrt(), // zeta = 1.0
            mass: 1.0,
            ..Default::default()
        };
        let sample = solve_spring(&params, 0.0, 1.0, 5.0);
        assert!(sample.settled, "should settle after 5s");
        assert!((sample.value - 1.0).abs() < 0.01);
    }

    #[test]
    fn underdamped_oscillates() {
        let params = SpringParams::default(); // zeta ≈ 0.5
        // At early time, should overshoot
        let early = solve_spring(&params, 0.0, 1.0, 0.3);
        // At late time, should settle
        let late = solve_spring(&params, 0.0, 1.0, 10.0);
        assert!(!early.settled);
        assert!(late.settled, "should settle after 10s");
        assert!((late.value - 1.0).abs() < 0.01);
    }

    #[test]
    fn overdamped_no_overshoot() {
        let params = SpringParams {
            stiffness: 100.0,
            damping: 40.0, // zeta = 2.0, overdamped
            mass: 1.0,
            ..Default::default()
        };
        // Sample at multiple points, value should always be between origin and target
        for i in 0..100 {
            let t = i as f64 * 0.1;
            let sample = solve_spring(&params, 0.0, 1.0, t);
            assert!(
                sample.value >= -0.001 && sample.value <= 1.001,
                "overdamped should not overshoot at t={t}: got {}",
                sample.value
            );
        }
    }

    #[test]
    fn velocity_carryover() {
        let params = SpringParams {
            initial_velocity: 5.0,
            ..Default::default()
        };
        let sample = solve_spring(&params, 0.0, 1.0, 0.0);
        assert!((sample.velocity - 5.0).abs() < 1e-6);
    }

    #[test]
    fn zero_displacement_with_velocity() {
        let params = SpringParams {
            initial_velocity: 2.0,
            ..Default::default()
        };
        // Already at target but has velocity — should move away then come back
        let sample = solve_spring(&params, 1.0, 1.0, 0.05);
        assert!(sample.value > 1.0, "velocity should push past target");
    }
}
