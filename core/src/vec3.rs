use std::ops::{Add, Mul, Sub};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }

    pub fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    pub fn distance(&self, other: &Vec3) -> f64 {
        (*self - *other).length()
    }

    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// A unit vector in the same direction, or `Vec3::ZERO` for a
    /// (near-)zero-length input — the caller decides what a degenerate
    /// direction means for it, this never produces a NaN/Inf.
    pub fn normalized(self) -> Vec3 {
        let len = self.length();
        if len < 1e-12 {
            Vec3::ZERO
        } else {
            self * (1.0 / len)
        }
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, rhs: Vec3) -> Vec3 {
        Vec3::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, rhs: Vec3) -> Vec3 {
        Vec3::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, rhs: f64) -> Vec3 {
        Vec3::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: Vec3, b: Vec3) {
        assert!(
            a.distance(&b) < 1e-9,
            "expected {:?} to approximately equal {:?}",
            a,
            b
        );
    }

    #[test]
    fn cross_of_x_and_y_axes_is_z_axis() {
        approx_eq(
            Vec3::new(1.0, 0.0, 0.0).cross(Vec3::new(0.0, 1.0, 0.0)),
            Vec3::new(0.0, 0.0, 1.0),
        );
    }

    #[test]
    fn cross_is_anticommutative() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(-2.0, 0.5, 4.0);
        approx_eq(a.cross(b), b.cross(a) * -1.0);
    }

    #[test]
    fn cross_of_parallel_vectors_is_zero() {
        let a = Vec3::new(2.0, -1.0, 3.0);
        approx_eq(a.cross(a * 2.5), Vec3::ZERO);
    }

    #[test]
    fn cross_result_is_perpendicular_to_both_inputs() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, -1.0, 2.0);
        let c = a.cross(b);
        assert!(c.dot(a).abs() < 1e-9);
        assert!(c.dot(b).abs() < 1e-9);
    }

    #[test]
    fn normalized_has_unit_length_and_same_direction() {
        let a = Vec3::new(3.0, 4.0, 0.0);
        let n = a.normalized();
        assert!((n.length() - 1.0).abs() < 1e-9);
        approx_eq(n, Vec3::new(0.6, 0.8, 0.0));
    }

    #[test]
    fn normalized_of_zero_is_zero_not_nan() {
        let n = Vec3::ZERO.normalized();
        assert!(n.is_finite());
        approx_eq(n, Vec3::ZERO);
    }
}
