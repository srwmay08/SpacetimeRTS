// ============================================================================
// File: client/src/tuner.rs
// ============================================================================
// ----------------------------------------------------------------------------
// WEAPON & SPELL TUNER WORKBENCH & TF2 COMEDIC COMBAT FEEDBACK
// ----------------------------------------------------------------------------
// Provides:
// 1. In-game Weapon & Spell Tuner Workbench GUI (toggleable via F6 or 'tuner' console command)
//    - Adjusts grips, categories, damage types, base damage, reach, knockback, velocities, gravity, and blast radius in real time.
//    - Live equips custom-tuned weapons directly to the player's loadout for immediate playtesting.
//    - Exports tuned presets as clean Rust definitions to the game logs and console.
// 2. TF2-style Comic Damage Floaters & Popups:
//    - Bouncing 3D billboard text popups for CRIT! (red/gold), MINI-CRIT! (amber), BONK!, CLANG!, KABOOM!, and numeric damage.

use bevy::prelude::{Transform as BevyTransform, *};
use bevy::window::{CursorGrabMode, PrimaryWindow};
use tracing::info;

use spacetime_rts_logic::{
    WeaponGrip, WeaponCategory, DamageType, ProjectileKind,
    create_weapon,
};
use crate::components::*;
use crate::core::*;
use crate::weapons::{WeaponState, WeaponType};

// ----------------------------------------------------------------------------
// TUNER PRESETS CATALOG
// ----------------------------------------------------------------------------

pub const TUNER_PRESETS: &[&str] = &[
    "Unarmed",
    "1h Tiger Claws",
    "1h Black Jack",
    "1h Sword",
    "1h Hammer",
    "1h Axe",
    "2h Sword",
    "2h Hammer",
    "2h Axe",
    "Polearm Spear",
    "Polearm Javelin - Thrown",
    "Polearm Trident",
    "1h Ranged Hand Crossbow",
    "1h Ranged Revolver",
    "2h Ranged Long Bow",
    "2h Ranged Shotgun",
    "2h Ranged Sniper Rifle",
    "1h Ranged Wand",
    "1h Ranged Orb",
    "2h Ranged Runestaff",
    "Frying Pan",
    "Holy Mackerel",
    "Bouncy Bomb Launcher",
];


// ----------------------------------------------------------------------------
// TUNER RESOURCE & STATE
// ----------------------------------------------------------------------------

#[derive(Resource)]
pub struct WeaponTunerState {
    pub is_open: bool,
    pub current_preset_idx: usize,
    pub weapon_name: String,
    pub grip_idx: usize,
    pub category_idx: usize,
    pub damage_type_idx: usize,
    pub base_damage: f32,
    pub attack_speed: f32,
    pub reach_meters: f32,
    pub armor_penetration: f32,
    pub knockback_force: f32, // TF2 comedic launch impulse
    pub projectile_kind_idx: usize,
    pub muzzle_velocity: f32,
    pub gravity: f32,
    pub blast_radius: f32,
    pub pellet_count: u32,
    pub spread_radians: f32,
}

impl Default for WeaponTunerState {
    fn default() -> Self {
        let mut state = Self {
            is_open: false,
            current_preset_idx: 0,
            weapon_name: "Frying Pan".into(),
            grip_idx: 0,        // OneHanded
            category_idx: 9,    // Comedic
            damage_type_idx: 7, // CartoonBonk
            base_damage: 45.0,
            attack_speed: 1.8,
            reach_meters: 1.2,
            armor_penetration: 0.15,
            knockback_force: 65.0,
            projectile_kind_idx: 0, // None
            muzzle_velocity: 0.0,
            gravity: 0.0,
            blast_radius: 0.0,
            pellet_count: 1,
            spread_radians: 0.0,
        };
        state.load_preset(0);
        state
    }
}

impl WeaponTunerState {
    pub fn load_preset(&mut self, idx: usize) {
        if idx >= TUNER_PRESETS.len() {
            return;
        }
        self.current_preset_idx = idx;
        let preset_name = TUNER_PRESETS[idx];

        if let Some(def) = create_weapon(preset_name) {
            self.weapon_name = def.name.clone();
            self.grip_idx = match def.grip {
                WeaponGrip::OneHanded => 0,
                WeaponGrip::TwoHanded => 1,
                WeaponGrip::Polearm => 2,
                WeaponGrip::Versatile => 3,
            };
            self.category_idx = match def.category {
                WeaponCategory::Edged => 0,
                WeaponCategory::Pointed => 1,
                WeaponCategory::Blunt => 2,
                WeaponCategory::TwoHanded => 3,
                WeaponCategory::Polearm => 4,
                WeaponCategory::Brawling => 5,
                WeaponCategory::Missile => 6,
                WeaponCategory::Firearm => 7,
                WeaponCategory::Runestaff => 8,
                WeaponCategory::Comedic => 9,
            };
            self.damage_type_idx = match def.damage_type {
                DamageType::Slashing => 0,
                DamageType::Piercing => 1,
                DamageType::Bludgeoning => 2,
                DamageType::Ballistic => 3,
                DamageType::PelletSpread => 4,
                DamageType::ArcaneForce => 5,
                DamageType::FireSplash => 6,
                DamageType::CartoonBonk => 7,
            };
            self.base_damage = def.base_damage;
            self.attack_speed = def.attack_speed;
            self.reach_meters = def.reach_meters;
            self.armor_penetration = def.armor_penetration;
            self.knockback_force = def.knockback_force;

            if let Some(proj) = def.projectile_profile {
                self.projectile_kind_idx = match proj.kind {
                    ProjectileKind::Arrow => 1,
                    ProjectileKind::HandCrossbowBolt => 2,
                    ProjectileKind::RevolverBullet => 3,
                    ProjectileKind::ShotgunPellet => 4,
                    ProjectileKind::SniperBullet => 5,
                    ProjectileKind::MagicMissile => 6,
                    ProjectileKind::FireballBall => 7,
                    ProjectileKind::BouncyBomb => 8,
                    ProjectileKind::Rocket => 9,
                    _ => 1,
                };
                self.muzzle_velocity = proj.muzzle_velocity;
                self.gravity = proj.gravity;
                self.blast_radius = proj.blast_radius;
                self.pellet_count = proj.pellet_count;
                self.spread_radians = proj.spread_radians;
            } else {
                self.projectile_kind_idx = 0; // None
                self.muzzle_velocity = 0.0;
                self.gravity = 0.0;
                self.blast_radius = 0.0;
                self.pellet_count = 1;
                self.spread_radians = 0.0;
            }
        }
    }

    pub fn grip_str(&self) -> &'static str {
        match self.grip_idx {
            0 => "OneHanded",
            1 => "TwoHanded",
            2 => "Polearm",
            _ => "Versatile",
        }
    }

    pub fn category_str(&self) -> &'static str {
        match self.category_idx {
            0 => "Edged",
            1 => "Pointed",
            2 => "Blunt",
            3 => "TwoHanded",
            4 => "Polearm",
            5 => "Brawling",
            6 => "Missile",
            7 => "Firearm",
            8 => "Runestaff",
            _ => "Comedic",
        }
    }

    pub fn damage_type_str(&self) -> &'static str {
        match self.damage_type_idx {
            0 => "Slashing",
            1 => "Piercing",
            2 => "Bludgeoning",
            3 => "Ballistic",
            4 => "PelletSpread",
            5 => "ArcaneForce",
            6 => "FireSplash",
            _ => "CartoonBonk",
        }
    }

    pub fn projectile_kind_str(&self) -> &'static str {
        match self.projectile_kind_idx {
            0 => "None (Melee)",
            1 => "Arrow",
            2 => "HandCrossbowBolt",
            3 => "RevolverBullet",
            4 => "ShotgunPellet",
            5 => "SniperBullet",
            6 => "MagicMissile",
            7 => "FireballBall",
            8 => "BouncyBomb",
            _ => "Rocket",
        }
    }

    pub fn export_rust_code(&self) -> String {
        format!(
            "// Custom Weapon Definition Export\n\
            WeaponDef {{\n\
            \x20   id: \"{}\".into(),\n\
            \x20   name: \"{}\".into(),\n\
            \x20   grip: WeaponGrip::{},\n\
            \x20   category: WeaponCategory::{},\n\
            \x20   damage_type: DamageType::{},\n\
            \x20   base_damage: {:.1},\n\
            \x20   attack_speed: {:.2},\n\
            \x20   reach_meters: {:.1},\n\
            \x20   armor_penetration: {:.2},\n\
            \x20   durability: 200,\n\
            \x20   max_durability: 200,\n\
            \x20   magazine_capacity: {},\n\
            \x20   current_ammo: {},\n\
            \x20   projectile_profile: {},\n\
            \x20   knockback_force: {:.1},\n\
            }}",
            self.weapon_name.to_lowercase().replace(' ', "_"),
            self.weapon_name,
            self.grip_str(),
            self.category_str(),
            self.damage_type_str(),
            self.base_damage,
            self.attack_speed,
            self.reach_meters,
            self.armor_penetration,
            if self.projectile_kind_idx > 0 { "Some(6)" } else { "None" },
            if self.projectile_kind_idx > 0 { 6 } else { 0 },
            if self.projectile_kind_idx > 0 {
                format!(
                    "Some(ProjectileProfile {{\n\
                    \x20       kind: ProjectileKind::{},\n\
                    \x20       muzzle_velocity: {:.1},\n\
                    \x20       gravity: {:.1},\n\
                    \x20       drag: 0.001,\n\
                    \x20       spread_radians: {:.3},\n\
                    \x20       pellet_count: {},\n\
                    \x20       blast_radius: {:.1},\n\
                    \x20       is_slow_projectile: false,\n\
                    \x20   }})",
                    self.projectile_kind_str(),
                    self.muzzle_velocity,
                    self.gravity,
                    self.spread_radians,
                    self.pellet_count,
                    self.blast_radius
                )
            } else {
                "None".to_string()
            },
            self.knockback_force
        )
    }
}

// ----------------------------------------------------------------------------
// TUNER ECS MARKERS & ACTIONS
// ----------------------------------------------------------------------------

#[derive(Component)]
pub struct TunerRoot;

#[derive(Component)]
pub struct TunerFieldDisplay(pub String);

#[derive(Component, Clone, Debug)]
pub enum TunerAction {
    PrevPreset,
    NextPreset,
    CycleGrip,
    CycleCategory,
    CycleDamageType,
    CycleProjectileKind,
    AdjustDamage(f32),
    AdjustSpeed(f32),
    AdjustReach(f32),
    AdjustArmorPen(f32),
    AdjustKnockback(f32),
    AdjustVelocity(f32),
    AdjustGravity(f32),
    AdjustBlastRadius(f32),
    EquipMainHand,
    EquipOffHand,
    ExportPreset,
    CloseTuner,
}

// ----------------------------------------------------------------------------
// COMIC FLOATING DAMAGE POPUP ECS
// ----------------------------------------------------------------------------

#[derive(Component)]
pub struct ComicTextFloater {
    pub timer: Timer,
    pub velocity: Vec3,
    pub is_crit: bool,
}

// ----------------------------------------------------------------------------
// UI SETUP SYSTEM
// ----------------------------------------------------------------------------

pub fn setup_tuner_ui(mut commands: Commands) {
    let root = commands
        .spawn((
            TunerRoot,
            NodeBundle {
                style: Style {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    left: Val::Percent(15.0),
                    right: Val::Percent(15.0),
                    top: Val::Percent(10.0),
                    bottom: Val::Percent(10.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                background_color: Color::srgba(0.08, 0.09, 0.12, 0.94).into(),
                border_color: Color::srgb(0.92, 0.45, 0.12).into(), // TF2 orange border
                ..default()
            },
        ))
        .id();

    // 1. Header Bar: Title, Preset Picker, Close Button
    commands.entity(root).with_children(|parent| {
        parent
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
                ..default()
            })
            .with_children(|header| {
                header.spawn(TextBundle::from_section(
                    "🛠️ WEAPON & SPELL TUNER WORKBENCH (TF2 COMEDY & BALLISTICS LAB)",
                    TextStyle {
                        font_size: 16.0,
                        color: Color::srgb(0.95, 0.65, 0.20),
                        ..default()
                    },
                ));

                // Preset selector row: [<] [Preset Name] [>]
                header
                    .spawn(NodeBundle {
                        style: Style {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(6.0),
                            ..default()
                        },
                        ..default()
                    })
                    .with_children(|picker| {
                        spawn_button(picker, "<", TunerAction::PrevPreset, 28.0, 24.0);
                        picker.spawn((
                            TunerFieldDisplay("preset_name".into()),
                            TextBundle::from_section(
                                "Preset: Frying Pan",
                                TextStyle {
                                    font_size: 13.0,
                                    color: Color::WHITE,
                                    ..default()
                                },
                            ),
                        ));
                        spawn_button(picker, ">", TunerAction::NextPreset, 28.0, 24.0);
                    });

                // Close Button
                spawn_button(header, " [X] ", TunerAction::CloseTuner, 36.0, 26.0);
            });

        // 2. Main 2-Column Body
        parent
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    column_gap: Val::Px(16.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|body| {
                // Column 1: Core Weapon Attributes & Kinematics
                body.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(50.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        ..default()
                    },
                    background_color: Color::srgba(0.12, 0.14, 0.18, 0.85).into(),
                    ..default()
                })
                .with_children(|col1| {
                    col1.spawn(TextBundle::from_section(
                        "--- CORE WEAPON ATTRIBUTES ---",
                        TextStyle { font_size: 13.0, color: Color::srgb(0.4, 0.8, 1.0), ..default() },
                    ));

                    // Grip Row
                    spawn_cycle_row(col1, "Grip:", "grip_val", "OneHanded", TunerAction::CycleGrip);
                    // Category Row
                    spawn_cycle_row(col1, "Category:", "cat_val", "Comedic", TunerAction::CycleCategory);
                    // Damage Type Row
                    spawn_cycle_row(col1, "Damage Type:", "dmg_type_val", "CartoonBonk", TunerAction::CycleDamageType);

                    // Stepper Rows
                    spawn_stepper_row(col1, "Base Damage:", "base_damage_val", "45.0 HP", TunerAction::AdjustDamage(-5.0), TunerAction::AdjustDamage(5.0));
                    spawn_stepper_row(col1, "Attack Speed:", "atk_speed_val", "1.80 /s", TunerAction::AdjustSpeed(-0.1), TunerAction::AdjustSpeed(0.1));
                    spawn_stepper_row(col1, "Reach / Range:", "reach_val", "1.2 m", TunerAction::AdjustReach(-0.2), TunerAction::AdjustReach(0.2));
                    spawn_stepper_row(col1, "Armor Pen:", "armor_pen_val", "15 %", TunerAction::AdjustArmorPen(-0.05), TunerAction::AdjustArmorPen(0.05));
                    spawn_stepper_row(col1, "Knockback Force:", "knockback_val", "65.0 m/s (YEET)", TunerAction::AdjustKnockback(-5.0), TunerAction::AdjustKnockback(5.0));
                });

                // Column 2: Ballistics, Spells & Comedic Effects
                body.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(50.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        ..default()
                    },
                    background_color: Color::srgba(0.12, 0.14, 0.18, 0.85).into(),
                    ..default()
                })
                .with_children(|col2| {
                    col2.spawn(TextBundle::from_section(
                        "--- SPELL & BALLISTIC ATTRIBUTES ---",
                        TextStyle { font_size: 13.0, color: Color::srgb(1.0, 0.4, 0.8), ..default() },
                    ));

                    // Projectile Kind Cycle Row
                    spawn_cycle_row(col2, "Projectile Kind:", "proj_kind_val", "None (Melee)", TunerAction::CycleProjectileKind);

                    // Ballistics Stepper Rows
                    spawn_stepper_row(col2, "Muzzle Velocity:", "vel_val", "0.0 m/s", TunerAction::AdjustVelocity(-10.0), TunerAction::AdjustVelocity(10.0));
                    spawn_stepper_row(col2, "Trajectory Gravity:", "grav_val", "0.0 m/s²", TunerAction::AdjustGravity(-0.5), TunerAction::AdjustGravity(0.5));
                    spawn_stepper_row(col2, "Blast AoE Radius:", "blast_val", "0.0 m", TunerAction::AdjustBlastRadius(-0.5), TunerAction::AdjustBlastRadius(0.5));

                    // TF2 Comedic Guide / Info Box
                    col2.spawn(NodeBundle {
                        style: Style {
                            margin: UiRect::top(Val::Px(12.0)),
                            padding: UiRect::all(Val::Px(8.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            ..default()
                        },
                        background_color: Color::srgba(0.05, 0.05, 0.07, 0.9).into(),
                        ..default()
                    }).with_children(|info_box| {
                        info_box.spawn(TextBundle::from_section(
                            "💡 TF2 COMEDIC COMBAT NOTES:",
                            TextStyle { font_size: 11.0, color: Color::srgb(0.9, 0.8, 0.2), ..default() },
                        ));
                        info_box.spawn(TextBundle::from_section(
                            "• Knockback > 70 m/s triggers 'YEET' launch velocity on hit.\n\
                            • CartoonBonk damage causes 'CLANG!' with Pan, 'BONK!' with Fish.\n\
                            • Critical strikes multiply knockback by 2.2x and trigger 'KABOOM!'.\n\
                            • Press [F6] anytime to toggle this workbench.",
                            TextStyle { font_size: 10.0, color: Color::srgb(0.75, 0.75, 0.75), ..default() },
                        ));
                    });
                });
            });

        // 3. Bottom Action Bar: Equip, Spawn, Export
        parent
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceEvenly,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(6.0)),
                    ..default()
                },
                background_color: Color::srgba(0.05, 0.06, 0.08, 0.9).into(),
                ..default()
            })
            .with_children(|bottom| {
                spawn_action_button(bottom, "⚡ EQUIP MAIN-HAND", TunerAction::EquipMainHand, Color::srgb(0.15, 0.55, 0.25));
                spawn_action_button(bottom, "⚡ EQUIP OFF-HAND", TunerAction::EquipOffHand, Color::srgb(0.20, 0.45, 0.65));
                spawn_action_button(bottom, "💾 EXPORT PRESET", TunerAction::ExportPreset, Color::srgb(0.75, 0.40, 0.10));
            });
    });
}

fn spawn_button(parent: &mut ChildBuilder, text: &str, action: TunerAction, width: f32, height: f32) {
    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Px(width),
                    height: Val::Px(height),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                background_color: Color::srgb(0.22, 0.25, 0.32).into(),
                border_color: Color::srgb(0.4, 0.45, 0.55).into(),
                ..default()
            },
            TunerActionComponent(action),
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                text,
                TextStyle { font_size: 12.0, color: Color::WHITE, ..default() },
            ));
        });
}

fn spawn_action_button(parent: &mut ChildBuilder, text: &str, action: TunerAction, bg: Color) {
    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                background_color: bg.into(),
                border_color: Color::WHITE.into(),
                ..default()
            },
            TunerActionComponent(action),
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                text,
                TextStyle { font_size: 13.0, color: Color::WHITE, ..default() },
            ));
        });
}

fn spawn_cycle_row(parent: &mut ChildBuilder, label: &str, field_id: &str, initial_val: &str, action: TunerAction) {
    parent
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .with_children(|row| {
            row.spawn(TextBundle::from_section(
                label,
                TextStyle { font_size: 12.0, color: Color::srgb(0.8, 0.8, 0.8), ..default() },
            ));

            row.spawn(NodeBundle {
                style: Style { align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() },
                ..default()
            })
            .with_children(|val_box| {
                val_box.spawn((
                    TunerFieldDisplay(field_id.into()),
                    TextBundle::from_section(
                        initial_val,
                        TextStyle { font_size: 12.0, color: Color::srgb(1.0, 0.9, 0.4), ..default() },
                    ),
                ));
                spawn_button(val_box, "Cycle", action, 48.0, 22.0);
            });
        });
}

fn spawn_stepper_row(
    parent: &mut ChildBuilder,
    label: &str,
    field_id: &str,
    initial_val: &str,
    dec_action: TunerAction,
    inc_action: TunerAction,
) {
    parent
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .with_children(|row| {
            row.spawn(TextBundle::from_section(
                label,
                TextStyle { font_size: 12.0, color: Color::srgb(0.8, 0.8, 0.8), ..default() },
            ));

            row.spawn(NodeBundle {
                style: Style { align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() },
                ..default()
            })
            .with_children(|stepper| {
                stepper.spawn((
                    TunerFieldDisplay(field_id.into()),
                    TextBundle::from_section(
                        initial_val,
                        TextStyle { font_size: 12.0, color: Color::srgb(0.4, 1.0, 0.5), ..default() },
                    ),
                ));
                spawn_button(stepper, "-", dec_action, 24.0, 22.0);
                spawn_button(stepper, "+", inc_action, 24.0, 22.0);
            });
        });
}

#[derive(Component)]
pub struct TunerActionComponent(pub TunerAction);

// ----------------------------------------------------------------------------
// TOGGLE & INPUT SYSTEMS
// ----------------------------------------------------------------------------

pub fn toggle_tuner_ui(
    keys: Res<ButtonInput<KeyCode>>,
    mut tuner: ResMut<WeaponTunerState>,
    mut tuner_root_q: Query<&mut Style, With<TunerRoot>>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    camera_mode: Res<State<CameraMode>>,
) {
    if keys.just_pressed(KeyCode::F6) {
        tuner.is_open = !tuner.is_open;
        if let Ok(mut style) = tuner_root_q.get_single_mut() {
            style.display = if tuner.is_open { Display::Flex } else { Display::None };
        }

        if let Ok(mut window) = window_q.get_single_mut() {
            if tuner.is_open {
                window.cursor.grab_mode = CursorGrabMode::None;
                window.cursor.visible = true;
            } else if *camera_mode.get() == CameraMode::FPS {
                window.cursor.grab_mode = CursorGrabMode::Locked;
                window.cursor.visible = false;
            }
        }
    }
}

pub fn handle_tuner_interactions(
    mut interaction_q: Query<(&Interaction, &TunerActionComponent), (Changed<Interaction>, With<Button>)>,
    mut tuner: ResMut<WeaponTunerState>,
    mut tuner_root_q: Query<&mut Style, With<TunerRoot>>,
    mut active_equipped: ResMut<ActiveEquippedItem>,
    mut active_offhand: ResMut<ActiveOffHandItem>,
    mut weapon_state: ResMut<WeaponState>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    camera_mode: Res<State<CameraMode>>,
) {
    for (interaction, action_comp) in interaction_q.iter_mut() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match &action_comp.0 {
            TunerAction::PrevPreset => {
                let prev = if tuner.current_preset_idx == 0 {
                    TUNER_PRESETS.len() - 1
                } else {
                    tuner.current_preset_idx - 1
                };
                tuner.load_preset(prev);
            }
            TunerAction::NextPreset => {
                let next = (tuner.current_preset_idx + 1) % TUNER_PRESETS.len();
                tuner.load_preset(next);
            }
            TunerAction::CycleGrip => {
                tuner.grip_idx = (tuner.grip_idx + 1) % 4;
            }
            TunerAction::CycleCategory => {
                tuner.category_idx = (tuner.category_idx + 1) % 10;
            }
            TunerAction::CycleDamageType => {
                tuner.damage_type_idx = (tuner.damage_type_idx + 1) % 8;
            }
            TunerAction::CycleProjectileKind => {
                tuner.projectile_kind_idx = (tuner.projectile_kind_idx + 1) % 10;
            }
            TunerAction::AdjustDamage(delta) => {
                tuner.base_damage = (tuner.base_damage + delta).clamp(1.0, 500.0);
            }
            TunerAction::AdjustSpeed(delta) => {
                tuner.attack_speed = (tuner.attack_speed + delta).clamp(0.1, 8.0);
            }
            TunerAction::AdjustReach(delta) => {
                tuner.reach_meters = (tuner.reach_meters + delta).clamp(0.2, 300.0);
            }
            TunerAction::AdjustArmorPen(delta) => {
                tuner.armor_penetration = (tuner.armor_penetration + delta).clamp(0.0, 1.0);
            }
            TunerAction::AdjustKnockback(delta) => {
                tuner.knockback_force = (tuner.knockback_force + delta).clamp(0.0, 150.0);
            }
            TunerAction::AdjustVelocity(delta) => {
                tuner.muzzle_velocity = (tuner.muzzle_velocity + delta).clamp(0.0, 1000.0);
            }
            TunerAction::AdjustGravity(delta) => {
                tuner.gravity = (tuner.gravity + delta).clamp(0.0, 25.0);
            }
            TunerAction::AdjustBlastRadius(delta) => {
                tuner.blast_radius = (tuner.blast_radius + delta).clamp(0.0, 20.0);
            }
            TunerAction::EquipMainHand => {
                let weapon_name = tuner.weapon_name.clone();
                info!("TUNER: Equipping '{}' to Main-Hand loadout.", weapon_name);
                if weapon_name == "Unarmed" {
                    active_equipped.0 = None;
                    weapon_state.current_weapon = WeaponType::None;
                } else {
                    active_equipped.0 = Some(weapon_name.clone());
                    weapon_state.current_weapon = WeaponType::from_item_name(Some(&weapon_name));
                }
            }
            TunerAction::EquipOffHand => {
                let weapon_name = tuner.weapon_name.clone();
                info!("TUNER: Equipping '{}' to Off-Hand loadout.", weapon_name);
                if weapon_name == "Unarmed" {
                    active_offhand.0 = None;
                    weapon_state.offhand_weapon = WeaponType::None;
                } else {
                    active_offhand.0 = Some(weapon_name.clone());
                    weapon_state.offhand_weapon = WeaponType::from_item_name(Some(&weapon_name));
                }
            }
            TunerAction::ExportPreset => {
                let code = tuner.export_rust_code();
                info!("\n================== EXPORTED WEAPON PRESET ==================\n{}\n============================================================", code);
            }
            TunerAction::CloseTuner => {
                tuner.is_open = false;
                if let Ok(mut style) = tuner_root_q.get_single_mut() {
                    style.display = Display::None;
                }
                if let Ok(mut window) = window_q.get_single_mut() {
                    if *camera_mode.get() == CameraMode::FPS {
                        window.cursor.grab_mode = CursorGrabMode::Locked;
                        window.cursor.visible = false;
                    }
                }
            }
        }
    }
}

pub fn update_tuner_ui_display(
    tuner: Res<WeaponTunerState>,
    mut display_q: Query<(&TunerFieldDisplay, &mut Text)>,
) {
    if !tuner.is_open || !tuner.is_changed() {
        return;
    }

    for (display, mut text) in display_q.iter_mut() {
        let new_val = match display.0.as_str() {
            "preset_name" => format!("Preset: {}", tuner.weapon_name),
            "grip_val" => tuner.grip_str().to_string(),
            "cat_val" => tuner.category_str().to_string(),
            "dmg_type_val" => tuner.damage_type_str().to_string(),
            "base_damage_val" => format!("{:.1} HP", tuner.base_damage),
            "atk_speed_val" => format!("{:.2} /s", tuner.attack_speed),
            "reach_val" => format!("{:.1} m", tuner.reach_meters),
            "armor_pen_val" => format!("{:.0} %", tuner.armor_penetration * 100.0),
            "knockback_val" => {
                let tag = if tuner.knockback_force > 70.0 {
                    " (YEET)"
                } else if tuner.knockback_force > 40.0 {
                    " (LAUNCH)"
                } else {
                    ""
                };
                format!("{:.1} m/s{}", tuner.knockback_force, tag)
            }
            "proj_kind_val" => tuner.projectile_kind_str().to_string(),
            "vel_val" => format!("{:.1} m/s", tuner.muzzle_velocity),
            "grav_val" => format!("{:.1} m/s²", tuner.gravity),
            "blast_val" => format!("{:.1} m", tuner.blast_radius),
            _ => continue,
        };
        if text.sections[0].value != new_val {
            text.sections[0].value = new_val;
        }
    }
}

// ----------------------------------------------------------------------------
// COMIC DAMAGE FLOATER SYSTEMS (TF2 STYLE)
// ----------------------------------------------------------------------------

pub fn spawn_comic_damage_floater(
    commands: &mut Commands,
    pos: Vec3,
    label: &str,
    is_crit: bool,
) {
    let color = if is_crit {
        Color::srgb(1.0, 0.15, 0.15) // Bold TF2 Crit Red
    } else if label.starts_with("MINI-CRIT") {
        Color::srgb(1.0, 0.70, 0.10) // Mini-crit amber
    } else if label.contains("BONK") || label.contains("CLANG") {
        Color::srgb(0.20, 0.95, 0.85) // Comedic cyan
    } else {
        Color::srgb(1.0, 1.0, 1.0) // Normal white damage
    };

    let font_size = if is_crit { 24.0 } else { 16.0 };

    let random_offset_x = (rand::random::<f32>() - 0.5) * 0.4;
    let random_offset_z = (rand::random::<f32>() - 0.5) * 0.4;

    commands.spawn((
        ComicTextFloater {
            timer: Timer::from_seconds(1.2, TimerMode::Once),
            velocity: Vec3::new(random_offset_x * 2.0, 3.2, random_offset_z * 2.0),
            is_crit,
        },
        Text2dBundle {
            text: Text::from_section(
                label,
                TextStyle {
                    font_size,
                    color,
                    ..default()
                },
            ),
            transform: BevyTransform::from_xyz(pos.x + random_offset_x, pos.y + 1.2, pos.z + random_offset_z),
            ..default()
        },
    ));
}

pub fn update_comic_damage_floaters(
    mut commands: Commands,
    time: Res<Time>,
    mut floater_q: Query<(Entity, &mut ComicTextFloater, &mut BevyTransform, &mut Text)>,
) {
    for (entity, mut floater, mut transform, mut text) in floater_q.iter_mut() {
        floater.timer.tick(time.delta());

        if floater.timer.finished() {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        // Apply velocity and gravity arc
        let dt = time.delta_seconds();
        transform.translation += floater.velocity * dt;
        floater.velocity.y -= 4.0 * dt; // Gravity decay

        // Scale pop effect on crits
        if floater.is_crit {
            let progress = floater.timer.fraction();
            let scale = if progress < 0.2 {
                1.0 + (progress / 0.2) * 0.5 // Pop scale up
            } else {
                1.5 - ((progress - 0.2) / 0.8) * 0.5 // Settle
            };
            transform.scale = Vec3::splat(scale);
        }

        // Fade out in final 30% of lifetime
        let frac_remaining = 1.0 - floater.timer.fraction();
        if frac_remaining < 0.3 {
            let alpha = frac_remaining / 0.3;
            for section in text.sections.iter_mut() {
                section.style.color.set_alpha(alpha);
            }
        }
    }
}
