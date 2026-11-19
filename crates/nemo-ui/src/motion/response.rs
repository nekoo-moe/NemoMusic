//! The closed form of a spring step response: a pure function of time, so a view can ask for
//! any moment (and a snapshot can render it) without stepping a stored spring.

/// Position of a mass-spring-damper released at 0 and heading to 1, `t_s` seconds after the
/// start. `stiffness` is the spring constant (mass 1) and `ratio` the damping ratio: 1 or more
/// settles without overshoot, below 1 it overshoots by `exp(-pi * ratio / sqrt(1 - ratio^2))`
/// (about 9.5 % at 0.6).
#[must_use]
pub fn spring_response(t_s: f32, stiffness: f32, ratio: f32) -> f32 {
    if t_s <= 0.0 {
        return 0.0;
    }
    let omega = stiffness.max(f32::EPSILON).sqrt();
    let decay = (-ratio * omega * t_s).exp();
    if ratio >= 1.0 {
        return 1.0 - decay * (1.0 + omega * t_s);
    }
    let damped = omega * (1.0 - ratio * ratio).sqrt();
    let phase = damped * t_s;
    1.0 - decay * (phase.cos() + ratio * omega / damped * phase.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_starts_at_zero_and_ends_at_one() {
        assert!(spring_response(0.0, 380.0, 0.6).abs() < f32::EPSILON);
        assert!((spring_response(2.0, 380.0, 0.6) - 1.0).abs() < 1e-3);
        assert!((spring_response(2.0, 380.0, 1.0) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn a_damping_ratio_of_one_never_overshoots() {
        let max = (0..200_u16)
            .map(|i| spring_response(f32::from(i) / 100.0, 700.0, 1.0))
            .fold(0.0_f32, f32::max);
        assert!(max <= 1.0 + 1e-4, "peak {max}");
    }

    #[test]
    fn a_ratio_of_point_six_overshoots_about_nine_and_a_half_percent() {
        let max = (0..200_u16)
            .map(|i| spring_response(f32::from(i) / 200.0, 380.0, 0.6))
            .fold(0.0_f32, f32::max);
        assert!((max - 1.0 - 0.095).abs() < 0.01, "peak {max}");
    }
}
