//! Side panels for custom 3-D experiences: the experience's UI records how
//! wide its panels are in [`SideInsets`], and its orbit camera's viewport is
//! fitted between them, so the surface is centred in what is left.

use bevy::camera::Viewport;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::{ExperienceEntity, ExperienceStopped};
use crate::orbit::OrbitRig;

/// Screen width taken by side panels, in logical pixels.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct SideInsets {
    pub left: f32,
    pub right: f32,
}

impl SideInsets {
    /// Store `next`, touching the resource only when it changed.
    pub fn set(this: &mut ResMut<Self>, next: Self) {
        if **this != next {
            **this = next;
        }
    }
}

pub(super) struct SideInsetsPlugin;

impl Plugin for SideInsetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SideInsets>()
            .add_observer(|_: On<ExperienceStopped>, mut insets: ResMut<SideInsets>| {
                SideInsets::set(&mut insets, SideInsets::default());
            })
            .add_systems(Update, fit_cameras.run_if(any_with_component::<OrbitRig>));
    }
}

/// The physical viewport between the insets, or `None` (the whole window)
/// when there are no insets or they leave no room.
pub fn viewport(window: &Window, insets: SideInsets) -> Option<Viewport> {
    if insets == SideInsets::default() {
        return None;
    }
    let scale = window.scale_factor();
    let physical = window.physical_size();
    let left = (insets.left * scale) as u32;
    let right = (insets.right * scale) as u32;
    let width = physical.x.checked_sub(left + right)?;
    (width > 0 && physical.y > 0).then(|| Viewport {
        physical_position: UVec2::new(left, 0),
        physical_size: UVec2::new(width, physical.y),
        ..default()
    })
}

fn fit_cameras(
    insets: Res<SideInsets>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<&mut Camera, (With<OrbitRig>, With<ExperienceEntity>)>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let wanted = viewport(window, *insets);
    let key = |v: &Option<Viewport>| v.as_ref().map(|v| (v.physical_position, v.physical_size));
    for mut camera in &mut cameras {
        if key(&camera.viewport) != key(&wanted) {
            camera.viewport = wanted.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_sits_between_the_panels() {
        let mut window = Window::default();
        window.resolution.set_physical_resolution(1600, 900);
        assert!(viewport(&window, SideInsets::default()).is_none());
        let v = viewport(
            &window,
            SideInsets {
                left: 300.0,
                right: 400.0,
            },
        )
        .unwrap();
        assert_eq!(v.physical_position, UVec2::new(300, 0));
        assert_eq!(v.physical_size, UVec2::new(900, 900));
        let squeezed = SideInsets {
            left: 900.0,
            right: 900.0,
        };
        assert!(viewport(&window, squeezed).is_none());
    }
}
