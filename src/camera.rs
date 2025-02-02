use flecs_ecs::macros::Component;
use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Component)]
pub struct Camera(pub Mat4);

impl Camera {
    pub fn new(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self {
        Self(Mat4::orthographic_lh(left, right, bottom, top, near, far))
    }

    pub fn translate(&mut self, translation: Vec3) {
        let translation_matrix = Mat4::from_translation(translation);

        self.0 *= translation_matrix;
    }
}
