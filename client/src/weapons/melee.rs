// ============================================================================
// File: client/src/weapons/melee.rs
// ============================================================================
// ----------------------------------------------------------------------------
// DIRECTIONAL MELEE SWING KINEMATICS & TRANSFORMS
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use crate::core::{MeleeAttackPhase, MeleeSwingDirection};

/// Computes procedural position and rotation offsets for directional 4-way melee attacks
/// (Right slash, Left backhand, Overhead cleave, Forward thrust) across Windup, Release, Recovery.
pub fn compute_directional_melee_transform(
    direction: MeleeSwingDirection,
    phase: MeleeAttackPhase,
    windup_t: f32,
    release_t: f32,
    recovery_t: f32,
) -> (Vec3, Quat) {
    match direction {
        MeleeSwingDirection::Right => {
            // Left-to-Right diagonal slash (mouse flick right)
            match phase {
                MeleeAttackPhase::Windup => {
                    let ease = windup_t * windup_t;
                    let pos = Vec3::new(-0.18 * ease, 0.06 * ease, 0.08 * ease);
                    let rot = Quat::from_rotation_z(-0.35 * ease)
                        * Quat::from_rotation_y(-0.45 * ease)
                        * Quat::from_rotation_x(0.20 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Release => {
                    let ease = release_t;
                    let pos = Vec3::new(
                        -0.18 + 0.42 * ease,
                        0.06 - 0.22 * ease,
                        0.08 - 0.26 * (ease * std::f32::consts::PI).sin(),
                    );
                    let rot = Quat::from_rotation_z(-0.35 + 0.80 * ease)
                        * Quat::from_rotation_y(-0.45 + 0.90 * ease)
                        * Quat::from_rotation_x(0.20 - 0.50 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Recovery => {
                    let ease = 1.0 - recovery_t;
                    let pos = Vec3::new(0.24 * ease, -0.16 * ease, -0.04 * ease);
                    let rot = Quat::from_rotation_z(0.45 * ease)
                        * Quat::from_rotation_y(0.45 * ease)
                        * Quat::from_rotation_x(-0.30 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Idle => (Vec3::ZERO, Quat::IDENTITY),
            }
        }
        MeleeSwingDirection::Left => {
            // Right-to-Left backhand slash (mouse flick left)
            match phase {
                MeleeAttackPhase::Windup => {
                    let ease = windup_t * windup_t;
                    let pos = Vec3::new(0.18 * ease, 0.08 * ease, 0.08 * ease);
                    let rot = Quat::from_rotation_z(0.35 * ease)
                        * Quat::from_rotation_y(0.50 * ease)
                        * Quat::from_rotation_x(0.20 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Release => {
                    let ease = release_t;
                    let pos = Vec3::new(
                        0.18 - 0.42 * ease,
                        0.08 - 0.22 * ease,
                        0.08 - 0.26 * (ease * std::f32::consts::PI).sin(),
                    );
                    let rot = Quat::from_rotation_z(0.35 - 0.80 * ease)
                        * Quat::from_rotation_y(0.50 - 0.95 * ease)
                        * Quat::from_rotation_x(0.20 - 0.45 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Recovery => {
                    let ease = 1.0 - recovery_t;
                    let pos = Vec3::new(-0.24 * ease, -0.14 * ease, -0.04 * ease);
                    let rot = Quat::from_rotation_z(-0.45 * ease)
                        * Quat::from_rotation_y(-0.45 * ease)
                        * Quat::from_rotation_x(-0.25 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Idle => (Vec3::ZERO, Quat::IDENTITY),
            }
        }
        MeleeSwingDirection::Overhead => {
            // Downward vertical cleave (mouse pull down)
            match phase {
                MeleeAttackPhase::Windup => {
                    let ease = windup_t * windup_t;
                    let pos = Vec3::new(0.02 * ease, 0.24 * ease, 0.06 * ease);
                    let rot = Quat::from_rotation_x(0.85 * ease) * Quat::from_rotation_z(-0.12 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Release => {
                    let ease = release_t;
                    let pos = Vec3::new(
                        0.02,
                        0.24 - 0.46 * ease,
                        0.06 - 0.24 * (ease * std::f32::consts::PI).sin(),
                    );
                    let rot = Quat::from_rotation_x(0.85 - 1.65 * ease) * Quat::from_rotation_z(-0.12 + 0.12 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Recovery => {
                    let ease = 1.0 - recovery_t;
                    let pos = Vec3::new(0.02 * ease, -0.22 * ease, -0.02 * ease);
                    let rot = Quat::from_rotation_x(-0.80 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Idle => (Vec3::ZERO, Quat::IDENTITY),
            }
        }
        MeleeSwingDirection::Thrust => {
            // Linear forward stab (mouse push up)
            match phase {
                MeleeAttackPhase::Windup => {
                    let ease = windup_t * windup_t;
                    let pos = Vec3::new(0.02 * ease, -0.04 * ease, 0.16 * ease);
                    let rot = Quat::from_rotation_y(0.12 * ease) * Quat::from_rotation_x(-0.08 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Release => {
                    let ease = release_t;
                    let pos = Vec3::new(
                        0.02 * (1.0 - ease),
                        -0.04 + 0.06 * ease,
                        0.16 - 0.42 * ease,
                    );
                    let rot = Quat::from_rotation_y(0.12 * (1.0 - ease)) * Quat::from_rotation_x(-0.08 * (1.0 - ease));
                    (pos, rot)
                }
                MeleeAttackPhase::Recovery => {
                    let ease = 1.0 - recovery_t;
                    let pos = Vec3::new(0.0, 0.02 * ease, -0.26 * ease);
                    (pos, Quat::IDENTITY)
                }
                MeleeAttackPhase::Idle => (Vec3::ZERO, Quat::IDENTITY),
            }
        }
    }
}
