//! A simple orbit ("arcball"-style) camera.
//!
//! The camera always looks at `target` from a distance, its direction set by a
//! yaw/pitch pair. Dragging orbits, scrolling dollies in/out, and a modifier
//! drag pans the target across the view plane. [`Camera::fit`] frames a model's
//! bounding box; everything downstream (near/far planes, pan and dolly speed)
//! scales off the model radius so it behaves the same for a 2&nbsp;mm screw or a
//! 2&nbsp;m sculpture.

use glam::{Mat4, Vec3};

use crate::mesh::Aabb;

/// Vertical field of view, radians.
const FOV_Y: f32 = std::f32::consts::FRAC_PI_4;
/// Pitch is clamped just shy of the poles to avoid a degenerate up-vector.
const PITCH_LIMIT: f32 = 1.553; // ~89°

pub struct Camera {
    /// Point the camera looks at (world space).
    target: Vec3,
    /// Eye distance from `target`.
    distance: f32,
    /// Horizontal angle (radians).
    yaw: f32,
    /// Vertical angle (radians).
    pitch: f32,
    /// Scene scale (model bounding-sphere radius); drives speeds and clip planes.
    radius: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 3.0,
            yaw: std::f32::consts::FRAC_PI_4,
            pitch: 0.5,
            radius: 1.0,
        }
    }
}

impl Camera {
    /// Frame `aabb`: centre on it and back off far enough to see the whole
    /// bounding sphere, from a pleasant three-quarter angle.
    pub fn fit(&mut self, aabb: &Aabb) {
        let c = aabb.center();
        self.target = Vec3::new(c[0], c[1], c[2]);
        self.radius = aabb.radius();
        // Distance so the bounding sphere fits the vertical FOV, with margin.
        self.distance = self.radius / (FOV_Y * 0.5).sin() * 1.2;
        self.yaw = std::f32::consts::FRAC_PI_4;
        self.pitch = 0.5;
    }

    /// Orbit by a screen drag (pixels). Positive `dx` swings right, `dy` up.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        const SPEED: f32 = 0.008;
        self.yaw += dx * SPEED;
        self.pitch = (self.pitch + dy * SPEED).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Pan the target across the view plane by a screen drag (pixels).
    pub fn pan(&mut self, dx: f32, dy: f32, viewport_height: f32) {
        // Convert pixels to world units at the target plane: the view is
        // `2 * distance * tan(fov/2)` world units tall across `viewport_height`.
        let world_per_px = 2.0 * self.distance * (FOV_Y * 0.5).tan() / viewport_height.max(1.0);
        let (right, up, _) = self.basis();
        self.target += right * (-dx * world_per_px) + up * (dy * world_per_px);
    }

    /// Dolly in/out from a scroll delta (egui's `smooth_scroll_delta.y`).
    pub fn zoom(&mut self, scroll: f32) {
        self.distance *= (-scroll * 0.0015).exp();
        self.clamp_distance();
    }

    /// Dolly from a pinch/zoom gesture factor (egui's `zoom_delta`); a factor
    /// above 1 zooms in.
    pub fn pinch(&mut self, factor: f32) {
        if factor > 0.0 {
            self.distance /= factor;
            self.clamp_distance();
        }
    }

    fn clamp_distance(&mut self) {
        self.distance = self.distance.clamp(self.radius * 0.02, self.radius * 100.0);
    }

    /// Eye position in world space.
    fn eye(&self) -> Vec3 {
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        let (cy, sy) = (self.yaw.cos(), self.yaw.sin());
        let dir = Vec3::new(cy * cp, sp, sy * cp);
        self.target + dir * self.distance
    }

    /// Camera right/up/forward basis (forward points from eye toward target).
    fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let forward = (self.target - self.eye()).normalize_or_zero();
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(forward);
        (right, up, forward)
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }

    pub fn projection(&self, aspect: f32) -> Mat4 {
        let near = (self.distance - self.radius).max(self.radius * 0.01);
        let far = self.distance + self.radius * 4.0;
        Mat4::perspective_rh_gl(FOV_Y, aspect.max(1e-3), near, far)
    }
}
