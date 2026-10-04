//! An orbit, pan, zoom, and reset camera rig for 3-D surfaces (Z is up).
//!
//! Add [`OrbitPlugin`] and put an [`OrbitRig`] on a `Camera3d`: left-drag
//! orbits, right-drag pans, the wheel zooms, and F (or setting
//! [`OrbitRig::reset`]) returns to the rig's home view. Pointer input over
//! egui panels is ignored.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;

pub struct OrbitPlugin;

impl Plugin for OrbitPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, orbit_cameras);
    }
}

/// A view around `target`: `yaw` about Z, `pitch` above the ground plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitView {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
}

impl OrbitView {
    pub fn transform(&self) -> Transform {
        let dir = Vec3::new(
            self.pitch.cos() * self.yaw.cos(),
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
        );
        Transform::from_translation(self.target + dir * self.distance)
            .looking_at(self.target, Vec3::Z)
    }
}

#[derive(Component, Debug, Clone)]
#[require(Camera3d)]
pub struct OrbitRig {
    pub view: OrbitView,
    pub home: OrbitView,
    /// Return to `home` on the next update.
    pub reset: bool,
}

impl OrbitRig {
    pub fn new(home: OrbitView) -> Self {
        Self {
            view: home,
            home,
            reset: false,
        }
    }

    /// The rig and its starting transform, to spawn on a camera.
    pub fn bundle(home: OrbitView) -> (Self, Transform) {
        (Self::new(home), home.transform())
    }
}

fn orbit_cameras(
    mut rigs: Query<(&mut Transform, &mut OrbitRig)>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    egui: Option<Res<EguiWantsInput>>,
    windows: Query<&Window>,
) {
    let height = windows.iter().next().map_or(900.0, |w| w.height().max(1.0));
    let pointer_free = !egui.as_ref().is_some_and(|e| e.wants_any_pointer_input());
    let keys_free = !egui.as_ref().is_some_and(|e| e.wants_any_keyboard_input());
    for (mut transform, mut rig) in &mut rigs {
        if rig.reset || (keys_free && keys.just_pressed(KeyCode::KeyF)) {
            rig.view = rig.home;
            rig.reset = false;
        }
        if pointer_free {
            let view = &mut rig.view;
            let lines = match scroll.unit {
                MouseScrollUnit::Line => scroll.delta.y,
                MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
            };
            view.distance = (view.distance * 0.9f32.powf(lines)).clamp(2.0, 80.0);
            if buttons.pressed(MouseButton::Left) {
                view.yaw -= motion.delta.x * 0.008;
                view.pitch = (view.pitch + motion.delta.y * 0.008).clamp(-1.45, 1.45);
            } else if buttons.pressed(MouseButton::Right) {
                let right = Vec3::new(-view.yaw.sin(), view.yaw.cos(), 0.0);
                let k = view.distance / height;
                view.target += (-motion.delta.x * right + motion.delta.y * Vec3::Z) * k;
            }
        }
        let next = rig.view.transform();
        if *transform != next {
            *transform = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_view_looks_at_the_target_from_above() {
        let view = OrbitView {
            yaw: 0.8,
            pitch: 0.55,
            distance: 14.0,
            target: Vec3::new(0.0, 0.0, 1.5),
        };
        let t = view.transform();
        assert!((t.translation.distance(view.target) - 14.0).abs() < 1e-4);
        assert!(t.translation.z > view.target.z);
        let forward = t.forward();
        assert!(forward.dot((view.target - t.translation).normalize()) > 0.9999);
    }
}
