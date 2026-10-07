// ============================================================================
// File: client/src/creatures.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PROCEDURAL LOW-POLY CREATURE & NPC GEOMETRY GENERATOR
// ----------------------------------------------------------------------------
// Architectural Note:
// Generates stylized, flat-shaded, low-poly 3D models for fauna and NPCs
// (Deer, Wild Boar, Goblin Raider, Peasant Unit, Player Pet) matching the
// BlendSwap #9440 geometric aesthetic (AGENTS.md Directive #4).
//
// Replaces heavy, thousands-of-cube micro-voxel rasters with crisp polyhedral
// tapered prisms and faceted planes with exact face normals. Delivers ~95%
// fewer polygons, eliminating shadow cascade fill-rate bottlenecks while
// providing cohesive visual style across the fantasy RTS world.
// ----------------------------------------------------------------------------

use bevy::prelude::{Transform as BevyTransform, *};
use avian3d::prelude::*;
use crate::trees::LowPolyMeshBuilder;
use crate::components::*;
use crate::core::*;
use crate::audio_feedback::*;
use crate::tuner::spawn_comic_damage_floater;

// ----------------------------------------------------------------------------
// PROCEDURAL MESH BUILDER EXTENSION FOR CREATURE PRIMITIVES
// ----------------------------------------------------------------------------

trait LowPolyCreatureBuilderExt {
    fn add_faceted_box(&mut self, min: Vec3, max: Vec3, color: [f32; 4]);
    fn add_tapered_box(
        &mut self,
        min_b: Vec2, max_b: Vec2, y_b: f32,
        min_t: Vec2, max_t: Vec2, y_t: f32,
        color: [f32; 4],
    );
}

impl LowPolyCreatureBuilderExt for LowPolyMeshBuilder {
    /// Emits a flat-shaded box with 6 outward-facing planar quads (12 triangles).
    fn add_faceted_box(&mut self, min: Vec3, max: Vec3, color: [f32; 4]) {
        // Top Face (+Y)
        self.add_flat_quad(
            Vec3::new(min.x, max.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            [color[0] * 1.05, color[1] * 1.05, color[2] * 1.05, color[3]],
        );
        // Bottom Face (-Y)
        self.add_flat_quad(
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(min.x, min.y, max.z),
            [color[0] * 0.70, color[1] * 0.70, color[2] * 0.70, color[3]],
        );
        // East Face (+X)
        self.add_flat_quad(
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(max.x, max.y, max.z),
            [color[0] * 0.95, color[1] * 0.95, color[2] * 0.95, color[3]],
        );
        // West Face (-X)
        self.add_flat_quad(
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(min.x, max.y, max.z),
            Vec3::new(min.x, max.y, min.z),
            [color[0] * 0.85, color[1] * 0.85, color[2] * 0.85, color[3]],
        );
        // South Face (+Z)
        self.add_flat_quad(
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
            [color[0] * 1.00, color[1] * 1.00, color[2] * 1.00, color[3]],
        );
        // North Face (-Z)
        self.add_flat_quad(
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            [color[0] * 0.80, color[1] * 0.80, color[2] * 0.80, color[3]],
        );
    }

    /// Emits a tapered 4-sided frustum with independent base and top dimensions.
    fn add_tapered_box(
        &mut self,
        min_b: Vec2, max_b: Vec2, y_b: f32,
        min_t: Vec2, max_t: Vec2, y_t: f32,
        color: [f32; 4],
    ) {
        let b0 = Vec3::new(min_b.x, y_b, min_b.y);
        let b1 = Vec3::new(max_b.x, y_b, min_b.y);
        let b2 = Vec3::new(max_b.x, y_b, max_b.y);
        let b3 = Vec3::new(min_b.x, y_b, max_b.y);

        let t0 = Vec3::new(min_t.x, y_t, min_t.y);
        let t1 = Vec3::new(max_t.x, y_t, min_t.y);
        let t2 = Vec3::new(max_t.x, y_t, max_t.y);
        let t3 = Vec3::new(min_t.x, y_t, max_t.y);

        // Sides
        self.add_flat_quad(b0, b1, t1, t0, [color[0] * 0.82, color[1] * 0.82, color[2] * 0.82, color[3]]);
        self.add_flat_quad(b1, b2, t2, t1, [color[0] * 0.95, color[1] * 0.95, color[2] * 0.95, color[3]]);
        self.add_flat_quad(b2, b3, t3, t2, [color[0] * 1.00, color[1] * 1.00, color[2] * 1.00, color[3]]);
        self.add_flat_quad(b3, b0, t0, t3, [color[0] * 0.88, color[1] * 0.88, color[2] * 0.88, color[3]]);

        // Top & Bottom caps
        self.add_flat_quad(t3, t2, t1, t0, [color[0] * 1.08, color[1] * 1.08, color[2] * 1.08, color[3]]);
        self.add_flat_quad(b0, b1, b2, b3, [color[0] * 0.70, color[1] * 0.70, color[2] * 0.70, color[3]]);
    }
}

// ----------------------------------------------------------------------------
// 1. PROCEDURAL LOW-POLY DEER (RED DEER / FOREST STAG)
// ----------------------------------------------------------------------------

pub fn create_lowpoly_deer_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();

    let tawny = [0.84, 0.62, 0.42, 1.0];
    let shadow = [0.65, 0.45, 0.30, 1.0];
    let white = [0.96, 0.94, 0.90, 1.0];
    let cream = [0.92, 0.86, 0.78, 1.0];
    let black = [0.12, 0.12, 0.14, 1.0];
    let antler = [0.76, 0.72, 0.64, 1.0];

    // Slender 4-legged armature with articulated hocks and cloven hooves
    let leg_positions = [
        (-0.14, -0.26), // Back Left
        (0.14, -0.26),  // Back Right
        (-0.14, 0.22),  // Front Left
        (0.14, 0.22),   // Front Right
    ];

    for &(lx, lz) in &leg_positions {
        // Cloven black hoof
        builder.add_faceted_box(
            Vec3::new(lx - 0.04, 0.0, lz - 0.04),
            Vec3::new(lx + 0.04, 0.07, lz + 0.04),
            black,
        );
        // Slender lower leg prism (tapered 4-sided)
        builder.add_tapered_prism(
            Vec3::new(lx, 0.07, lz),
            Vec3::new(lx, 0.46, lz),
            0.030, 0.038, 4, shadow, false, false,
        );
        // Muscular upper leg / haunch
        builder.add_tapered_prism(
            Vec3::new(lx, 0.46, lz),
            Vec3::new(lx, 0.82, lz),
            0.038, 0.075, 4, tawny, false, true,
        );
    }

    // Sculpted muscular torso (tapered waist, cream underside)
    builder.add_tapered_box(
        Vec2::new(-0.16, -0.38), Vec2::new(0.16, 0.0), 0.70,
        Vec2::new(-0.18, -0.34), Vec2::new(0.18, 0.0), 1.08,
        tawny,
    );
    builder.add_tapered_box(
        Vec2::new(-0.18, 0.0), Vec2::new(0.18, 0.34), 0.70,
        Vec2::new(-0.19, 0.0), Vec2::new(0.19, 0.32), 1.14,
        tawny,
    );

    // Cream underbelly
    builder.add_faceted_box(
        Vec3::new(-0.13, 0.68, -0.32),
        Vec3::new(0.13, 0.74, 0.28),
        cream,
    );

    // White chest bib and tail
    builder.add_flat_quad(
        Vec3::new(-0.15, 0.76, 0.34),
        Vec3::new(0.15, 0.76, 0.34),
        Vec3::new(0.12, 1.10, 0.34),
        Vec3::new(-0.12, 1.10, 0.34),
        white,
    );
    builder.add_tapered_prism(
        Vec3::new(0.0, 1.02, -0.38),
        Vec3::new(0.0, 1.12, -0.48),
        0.045, 0.015, 4, white, true, true,
    );

    // Slender forward-angled neck
    builder.add_tapered_prism(
        Vec3::new(0.0, 1.04, 0.22),
        Vec3::new(0.0, 1.48, 0.38),
        0.11, 0.065, 5, tawny, false, true,
    );

    // Sculpted wedge head with muzzle and black nose pad
    builder.add_tapered_box(
        Vec2::new(-0.09, 0.32), Vec2::new(0.09, 0.50), 1.44,
        Vec2::new(-0.05, 0.50), Vec2::new(0.05, 0.62), 1.56,
        tawny,
    );
    builder.add_faceted_box(
        Vec3::new(-0.04, 1.45, 0.58),
        Vec3::new(0.04, 1.52, 0.64),
        black,
    );

    // Backward-angled faceted ears
    builder.add_flat_triangle(
        Vec3::new(-0.07, 1.55, 0.35),
        Vec3::new(-0.18, 1.70, 0.26),
        Vec3::new(-0.07, 1.62, 0.28),
        tawny,
    );
    builder.add_flat_triangle(
        Vec3::new(0.07, 1.55, 0.35),
        Vec3::new(0.07, 1.62, 0.28),
        Vec3::new(0.18, 1.70, 0.26),
        tawny,
    );

    // Branching low-poly antler rack (BlendSwap #9440 geometric style)
    let antler_angles = [(-1.0_f32), 1.0_f32];
    for &side in &antler_angles {
        let root = Vec3::new(side * 0.06, 1.56, 0.38);
        let fork = Vec3::new(side * 0.16, 1.84, 0.34);
        let tip1 = Vec3::new(side * 0.26, 2.15, 0.28);
        let tip2 = Vec3::new(side * 0.08, 1.98, 0.48);

        builder.add_tapered_prism(root, fork, 0.024, 0.018, 4, antler, true, false);
        builder.add_tapered_prism(fork, tip1, 0.018, 0.008, 4, antler, false, true);
        builder.add_tapered_prism(fork, tip2, 0.015, 0.006, 4, antler, false, true);
    }

    builder.build()
}

// Backwards-compatibility shim for zone editor and legacy systems
pub use create_lowpoly_deer_mesh as create_voxel_deer_mesh;

// ----------------------------------------------------------------------------
// 2. PROCEDURAL LOW-POLY BOAR (TUSKED RAIDER)
// ----------------------------------------------------------------------------

pub fn create_lowpoly_boar_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();

    let umber = [0.28, 0.16, 0.08, 1.0];
    let ochre = [0.65, 0.44, 0.22, 1.0];
    let highlight = [0.82, 0.62, 0.36, 1.0];
    let snout = [0.65, 0.45, 0.40, 1.0];
    let tusk = [0.96, 0.93, 0.86, 1.0];
    let hoof = [0.14, 0.12, 0.10, 1.0];

    // 4 Stocky legs
    let leg_offsets = [
        (-0.20, -0.28),
        (0.20, -0.28),
        (-0.20, 0.22),
        (0.20, 0.22),
    ];

    for &(lx, lz) in &leg_offsets {
        builder.add_faceted_box(
            Vec3::new(lx - 0.06, 0.0, lz - 0.06),
            Vec3::new(lx + 0.06, 0.06, lz + 0.06),
            hoof,
        );
        builder.add_tapered_prism(
            Vec3::new(lx, 0.06, lz),
            Vec3::new(lx, 0.36, lz),
            0.058, 0.085, 4, umber, false, true,
        );
    }

    // Heavy barrel torso with ochre flank markings
    builder.add_tapered_box(
        Vec2::new(-0.26, -0.44), Vec2::new(0.26, 0.28), 0.28,
        Vec2::new(-0.28, -0.40), Vec2::new(0.28, 0.24), 0.72,
        umber,
    );
    // Ochre flank stripes
    builder.add_flat_quad(
        Vec3::new(-0.285, 0.40, -0.32),
        Vec3::new(-0.285, 0.40, 0.16),
        Vec3::new(-0.285, 0.62, 0.12),
        Vec3::new(-0.285, 0.62, -0.28),
        ochre,
    );
    builder.add_flat_quad(
        Vec3::new(0.285, 0.40, 0.16),
        Vec3::new(0.285, 0.40, -0.32),
        Vec3::new(0.285, 0.62, -0.28),
        Vec3::new(0.285, 0.62, 0.12),
        ochre,
    );

    // Raised spine bristle crest
    builder.add_tapered_prism(
        Vec3::new(0.0, 0.70, -0.36),
        Vec3::new(0.0, 0.70, 0.20),
        0.06, 0.04, 3, highlight, true, true,
    );

    // Sloping wedge head & heavy jowls
    builder.add_tapered_box(
        Vec2::new(-0.22, 0.20), Vec2::new(0.22, 0.48), 0.32,
        Vec2::new(-0.14, 0.48), Vec2::new(0.14, 0.68), 0.66,
        umber,
    );

    // Snout pad
    builder.add_faceted_box(
        Vec3::new(-0.11, 0.36, 0.66),
        Vec3::new(0.11, 0.52, 0.75),
        snout,
    );

    // Upward-curved ivory tusks
    builder.add_tapered_prism(
        Vec3::new(-0.15, 0.38, 0.56),
        Vec3::new(-0.19, 0.58, 0.64),
        0.032, 0.008, 4, tusk, true, true,
    );
    builder.add_tapered_prism(
        Vec3::new(0.15, 0.38, 0.56),
        Vec3::new(0.19, 0.58, 0.64),
        0.032, 0.008, 4, tusk, true, true,
    );

    // Bristly ears
    builder.add_flat_triangle(
        Vec3::new(-0.16, 0.64, 0.32),
        Vec3::new(-0.26, 0.76, 0.26),
        Vec3::new(-0.14, 0.70, 0.24),
        umber,
    );
    builder.add_flat_triangle(
        Vec3::new(0.16, 0.64, 0.32),
        Vec3::new(0.14, 0.70, 0.24),
        Vec3::new(0.26, 0.76, 0.26),
        umber,
    );

    builder.build()
}

// Backwards-compatibility shim for zone editor and legacy systems
pub use create_lowpoly_boar_mesh as create_voxel_boar_mesh;

// ----------------------------------------------------------------------------
// 3. PROCEDURAL LOW-POLY GOBLIN (RAIDER / WARRIOR)
// ----------------------------------------------------------------------------

pub fn create_lowpoly_goblin_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();

    let skin = [0.38, 0.68, 0.22, 1.0];
    let iron = [0.46, 0.48, 0.52, 1.0];
    let iron_dark = [0.28, 0.30, 0.34, 1.0];
    let leather = [0.36, 0.22, 0.14, 1.0];
    let gold = [0.94, 0.78, 0.18, 1.0];
    let eye = [0.98, 0.92, 0.15, 1.0];
    let bone = [0.92, 0.88, 0.78, 1.0];

    // Armored boots
    builder.add_faceted_box(Vec3::new(-0.20, 0.0, -0.12), Vec3::new(-0.06, 0.12, 0.14), iron_dark);
    builder.add_faceted_box(Vec3::new(0.06, 0.0, -0.12), Vec3::new(0.20, 0.12, 0.14), iron_dark);

    // Squat muscular legs
    builder.add_tapered_prism(Vec3::new(-0.13, 0.12, 0.0), Vec3::new(-0.11, 0.44, 0.0), 0.075, 0.09, 4, skin, false, true);
    builder.add_tapered_prism(Vec3::new(0.13, 0.12, 0.0), Vec3::new(0.11, 0.44, 0.0), 0.075, 0.09, 4, skin, false, true);

    // Studded war belt with gold buckle & tassets
    builder.add_faceted_box(Vec3::new(-0.20, 0.44, -0.14), Vec3::new(0.20, 0.56, 0.14), leather);
    builder.add_faceted_box(Vec3::new(-0.07, 0.46, 0.13), Vec3::new(0.07, 0.54, 0.16), gold);

    // Segmented iron cuirass / breastplate
    builder.add_tapered_box(
        Vec2::new(-0.21, -0.13), Vec2::new(0.21, 0.13), 0.56,
        Vec2::new(-0.24, -0.15), Vec2::new(0.24, 0.15), 0.94,
        iron,
    );

    // Angular dual-tier pauldrons on shoulders
    builder.add_tapered_prism(Vec3::new(-0.24, 0.86, 0.0), Vec3::new(-0.36, 0.96, 0.0), 0.12, 0.08, 4, iron_dark, true, true);
    builder.add_tapered_prism(Vec3::new(0.24, 0.86, 0.0), Vec3::new(0.36, 0.96, 0.0), 0.12, 0.08, 4, iron_dark, true, true);

    // Muscular arms & iron bracers
    builder.add_tapered_prism(Vec3::new(-0.28, 0.84, 0.0), Vec3::new(-0.26, 0.46, 0.05), 0.07, 0.055, 4, skin, false, false);
    builder.add_tapered_prism(Vec3::new(0.28, 0.84, 0.0), Vec3::new(0.26, 0.46, 0.05), 0.07, 0.055, 4, skin, false, false);
    builder.add_faceted_box(Vec3::new(-0.31, 0.42, 0.0), Vec3::new(-0.21, 0.56, 0.10), iron);
    builder.add_faceted_box(Vec3::new(0.21, 0.42, 0.0), Vec3::new(0.31, 0.56, 0.10), iron);

    // Goblin head & jaw
    builder.add_tapered_box(
        Vec2::new(-0.16, -0.13), Vec2::new(0.16, 0.16), 0.94,
        Vec2::new(-0.14, -0.12), Vec2::new(0.14, 0.14), 1.24,
        skin,
    );

    // Lower jaw tusks
    builder.add_tapered_prism(Vec3::new(-0.08, 0.98, 0.15), Vec3::new(-0.09, 1.12, 0.17), 0.024, 0.006, 3, bone, true, true);
    builder.add_tapered_prism(Vec3::new(0.08, 0.98, 0.15), Vec3::new(0.09, 1.12, 0.17), 0.024, 0.006, 3, bone, true, true);

    // Glowing eyes
    builder.add_faceted_box(Vec3::new(-0.11, 1.08, 0.14), Vec3::new(-0.04, 1.14, 0.17), eye);
    builder.add_faceted_box(Vec3::new(0.04, 1.08, 0.14), Vec3::new(0.11, 1.14, 0.17), eye);

    // Large pointed lateral ears
    builder.add_flat_triangle(Vec3::new(-0.14, 1.06, 0.0), Vec3::new(-0.36, 1.18, -0.04), Vec3::new(-0.14, 1.18, -0.02), skin);
    builder.add_flat_triangle(Vec3::new(0.14, 1.06, 0.0), Vec3::new(0.14, 1.18, -0.02), Vec3::new(0.36, 1.18, -0.04), skin);

    // Horned iron helmet
    builder.add_tapered_box(
        Vec2::new(-0.17, -0.15), Vec2::new(0.17, 0.15), 1.20,
        Vec2::new(-0.13, -0.13), Vec2::new(0.13, 0.13), 1.36,
        iron_dark,
    );
    // Helmet horns
    builder.add_tapered_prism(Vec3::new(-0.14, 1.28, 0.0), Vec3::new(-0.28, 1.50, 0.08), 0.038, 0.008, 4, bone, true, true);
    builder.add_tapered_prism(Vec3::new(0.14, 1.28, 0.0), Vec3::new(0.28, 1.50, 0.08), 0.038, 0.008, 4, bone, true, true);

    builder.build()
}

// Backwards-compatibility shim for zone editor and legacy systems
pub use create_lowpoly_goblin_mesh as create_voxel_goblin_mesh;

// ----------------------------------------------------------------------------
// 3b. PROCEDURAL LOW-POLY TRAINING DUMMY (STRAW & TIMBER)
// ----------------------------------------------------------------------------

pub fn create_lowpoly_dummy_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();

    let wood_dark = [0.32, 0.20, 0.12, 1.0];
    let wood_light = [0.48, 0.32, 0.18, 1.0];
    let straw = [0.82, 0.72, 0.38, 1.0];
    let straw_dark = [0.68, 0.56, 0.28, 1.0];
    let rope = [0.38, 0.26, 0.16, 1.0];
    let target_red = [0.90, 0.18, 0.15, 1.0];
    let target_white = [0.95, 0.94, 0.90, 1.0];
    let iron_helmet = [0.30, 0.32, 0.36, 1.0];

    // Heavy cross-timber ground base
    builder.add_faceted_box(Vec3::new(-0.55, 0.0, -0.10), Vec3::new(0.55, 0.08, 0.10), wood_dark);
    builder.add_faceted_box(Vec3::new(-0.10, 0.0, -0.55), Vec3::new(0.10, 0.08, 0.55), wood_dark);

    // Thick vertical wooden mounting post (Y: 0.0 to 1.85)
    builder.add_tapered_prism(Vec3::new(0.0, 0.08, 0.0), Vec3::new(0.0, 1.85, 0.0), 0.11, 0.08, 6, wood_light, false, true);

    // Burlap straw-stuffed torso (Y: 0.55 to 1.42)
    builder.add_tapered_box(
        Vec2::new(-0.24, -0.16), Vec2::new(0.24, 0.16), 0.55,
        Vec2::new(-0.28, -0.18), Vec2::new(0.28, 0.18), 1.05,
        straw,
    );
    builder.add_tapered_box(
        Vec2::new(-0.28, -0.18), Vec2::new(0.28, 0.18), 1.05,
        Vec2::new(-0.22, -0.15), Vec2::new(0.22, 0.15), 1.42,
        straw,
    );

    // Thick rope bindings around waist and chest
    builder.add_faceted_box(Vec3::new(-0.29, 0.72, -0.19), Vec3::new(0.29, 0.78, 0.19), rope);
    builder.add_faceted_box(Vec3::new(-0.29, 1.15, -0.19), Vec3::new(0.29, 1.21, 0.19), rope);

    // Target bullseye on front of torso (+Z)
    // Outer red square
    builder.add_faceted_box(Vec3::new(-0.14, 0.88, 0.181), Vec3::new(0.14, 1.16, 0.186), target_red);
    // Middle white square
    builder.add_faceted_box(Vec3::new(-0.09, 0.93, 0.186), Vec3::new(0.09, 1.11, 0.191), target_white);
    // Center red bullseye
    builder.add_faceted_box(Vec3::new(-0.045, 0.975, 0.191), Vec3::new(0.045, 1.065, 0.196), target_red);

    // Horizontal crossbar sparring arms (through shoulders Y: 1.28)
    builder.add_faceted_box(Vec3::new(-0.75, 1.26, -0.06), Vec3::new(0.75, 1.34, 0.06), wood_light);
    // Straw padding bundles wrapped on ends of arms
    builder.add_tapered_prism(Vec3::new(-0.45, 1.30, 0.0), Vec3::new(-0.75, 1.30, 0.0), 0.08, 0.09, 6, straw_dark, true, true);
    builder.add_tapered_prism(Vec3::new(0.45, 1.30, 0.0), Vec3::new(0.75, 1.30, 0.0), 0.08, 0.09, 6, straw_dark, true, true);
    // Arm rope bands
    builder.add_faceted_box(Vec3::new(-0.62, 1.22, -0.09), Vec3::new(-0.58, 1.38, 0.09), rope);
    builder.add_faceted_box(Vec3::new(0.58, 1.22, -0.09), Vec3::new(0.62, 1.38, 0.09), rope);

    // Dummy Head (Straw sphere-like polyhedron, Y: 1.48 to 1.82)
    builder.add_tapered_box(
        Vec2::new(-0.14, -0.14), Vec2::new(0.14, 0.14), 1.48,
        Vec2::new(-0.16, -0.16), Vec2::new(0.16, 0.16), 1.68,
        straw,
    );
    builder.add_tapered_box(
        Vec2::new(-0.16, -0.16), Vec2::new(0.16, 0.16), 1.68,
        Vec2::new(-0.12, -0.12), Vec2::new(0.12, 0.12), 1.82,
        straw,
    );

    // Iron pot helmet on dummy head (Y: 1.74 to 1.88)
    builder.add_faceted_box(Vec3::new(-0.18, 1.74, -0.18), Vec3::new(0.18, 1.82, 0.18), iron_helmet);
    builder.add_faceted_box(Vec3::new(-0.14, 1.82, -0.14), Vec3::new(0.14, 1.88, 0.14), iron_helmet);

    builder.build()
}

// ----------------------------------------------------------------------------
// 4. PROCEDURAL LOW-POLY PEASANT (WORKER / VILLAGER)
// ----------------------------------------------------------------------------

pub fn create_lowpoly_peasant_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();

    let skin = [0.86, 0.72, 0.60, 1.0];
    let shirt = [0.22, 0.42, 0.85, 1.0];
    let pants = [0.32, 0.26, 0.20, 1.0];
    let hair = [0.28, 0.18, 0.10, 1.0];
    let boots = [0.18, 0.12, 0.08, 1.0];

    // Sturdy work boots
    builder.add_faceted_box(Vec3::new(-0.18, 0.0, -0.10), Vec3::new(-0.05, 0.14, 0.14), boots);
    builder.add_faceted_box(Vec3::new(0.05, 0.0, -0.10), Vec3::new(0.18, 0.14, 0.14), boots);

    // Trousers (4-sided tapered prisms)
    builder.add_tapered_prism(Vec3::new(-0.11, 0.14, 0.0), Vec3::new(-0.10, 0.52, 0.0), 0.068, 0.082, 4, pants, false, true);
    builder.add_tapered_prism(Vec3::new(0.11, 0.14, 0.0), Vec3::new(0.10, 0.52, 0.0), 0.068, 0.082, 4, pants, false, true);

    // Blue peasant tunic / shirt torso
    builder.add_tapered_box(
        Vec2::new(-0.19, -0.12), Vec2::new(0.19, 0.12), 0.52,
        Vec2::new(-0.22, -0.14), Vec2::new(0.22, 0.14), 0.98,
        shirt,
    );

    // Shoulders & Sleeves
    builder.add_tapered_prism(Vec3::new(-0.23, 0.90, 0.0), Vec3::new(-0.28, 0.58, 0.0), 0.075, 0.058, 4, shirt, true, false);
    builder.add_tapered_prism(Vec3::new(0.23, 0.90, 0.0), Vec3::new(0.28, 0.58, 0.0), 0.075, 0.058, 4, shirt, true, false);

    // Hands
    builder.add_faceted_box(Vec3::new(-0.31, 0.44, -0.04), Vec3::new(-0.25, 0.58, 0.06), skin);
    builder.add_faceted_box(Vec3::new(0.25, 0.44, -0.04), Vec3::new(0.31, 0.58, 0.06), skin);

    // Stylized head & neck
    builder.add_tapered_prism(Vec3::new(0.0, 0.96, 0.0), Vec3::new(0.0, 1.05, 0.0), 0.065, 0.060, 4, skin, false, false);
    builder.add_tapered_box(
        Vec2::new(-0.11, -0.10), Vec2::new(0.11, 0.12), 1.04,
        Vec2::new(-0.12, -0.11), Vec2::new(0.12, 0.11), 1.28,
        skin,
    );

    // Hair cap
    builder.add_tapered_box(
        Vec2::new(-0.13, -0.12), Vec2::new(0.13, 0.12), 1.20,
        Vec2::new(-0.11, -0.10), Vec2::new(0.11, 0.10), 1.34,
        hair,
    );

    builder.build()
}

// Backwards-compatibility shim for zone editor and legacy systems
pub use create_lowpoly_peasant_mesh as create_voxel_peasant_mesh;

// ----------------------------------------------------------------------------
// 5. PROCEDURAL LOW-POLY PET (COMPANION CANINE / FOX)
// ----------------------------------------------------------------------------

pub fn create_lowpoly_pet_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();

    let coat = [0.86, 0.52, 0.18, 1.0];
    let cream = [0.95, 0.90, 0.80, 1.0];
    let nose = [0.10, 0.10, 0.10, 1.0];
    let ears = [0.68, 0.38, 0.12, 1.0];

    // 4 legs
    let leg_pts = [
        (-0.10, -0.16),
        (0.10, -0.16),
        (-0.10, 0.14),
        (0.10, 0.14),
    ];
    for &(lx, lz) in &leg_pts {
        builder.add_faceted_box(
            Vec3::new(lx - 0.035, 0.0, lz - 0.035),
            Vec3::new(lx + 0.035, 0.04, lz + 0.035),
            cream,
        );
        builder.add_tapered_prism(
            Vec3::new(lx, 0.04, lz),
            Vec3::new(lx, 0.22, lz),
            0.032, 0.048, 4, coat, false, true,
        );
    }

    // Torso with cream underside
    builder.add_tapered_box(
        Vec2::new(-0.14, -0.24), Vec2::new(0.14, 0.20), 0.18,
        Vec2::new(-0.13, -0.22), Vec2::new(0.13, 0.18), 0.38,
        coat,
    );
    builder.add_flat_quad(
        Vec3::new(-0.11, 0.18, -0.20),
        Vec3::new(0.11, 0.18, -0.20),
        Vec3::new(0.11, 0.18, 0.18),
        Vec3::new(-0.11, 0.18, 0.18),
        cream,
    );

    // Head, muzzle & nose
    builder.add_tapered_box(
        Vec2::new(-0.09, 0.14), Vec2::new(0.09, 0.28), 0.30,
        Vec2::new(-0.08, 0.16), Vec2::new(0.08, 0.26), 0.48,
        coat,
    );
    builder.add_faceted_box(
        Vec3::new(-0.05, 0.30, 0.26),
        Vec3::new(0.05, 0.38, 0.38),
        cream,
    );
    builder.add_faceted_box(
        Vec3::new(-0.025, 0.34, 0.37),
        Vec3::new(0.025, 0.38, 0.40),
        nose,
    );

    // Perky triangular ears
    builder.add_flat_triangle(
        Vec3::new(-0.07, 0.46, 0.16),
        Vec3::new(-0.11, 0.58, 0.18),
        Vec3::new(-0.03, 0.48, 0.22),
        ears,
    );
    builder.add_flat_triangle(
        Vec3::new(0.07, 0.46, 0.16),
        Vec3::new(0.03, 0.48, 0.22),
        Vec3::new(0.11, 0.58, 0.18),
        ears,
    );

    // Upward wagging tail
    builder.add_tapered_prism(
        Vec3::new(0.0, 0.32, -0.22),
        Vec3::new(0.0, 0.46, -0.34),
        0.040, 0.015, 4, coat, true, true,
    );

    builder.build()
}

// Backwards-compatibility shim for zone editor and legacy systems
pub use create_lowpoly_pet_mesh as create_voxel_pet_mesh;

// ----------------------------------------------------------------------------
// CREATURE ENTITY CREATION & MESH CACHING
// ----------------------------------------------------------------------------

use bevy::render::view::RenderLayers;
use bevy::pbr::NotShadowCaster;
use crate::components::{NetworkEntity, LogicalPosition, LogicalRotation, Selectable, PeasantUnit, SelectionRing, RTSProxy};
use crate::core::{GameState, GameLayer};

/// Cached GPU mesh handles for fauna and humanoid NPCs.
pub struct CachedCreatureMeshes {
    pub deer: Handle<Mesh>,
    pub boar: Handle<Mesh>,
    pub goblin: Handle<Mesh>,
    pub peasant: Handle<Mesh>,
    pub pet: Handle<Mesh>,
    pub dummy: Handle<Mesh>,
}

impl CachedCreatureMeshes {
    pub fn new(meshes: &mut Assets<Mesh>) -> Self {
        Self {
            deer: meshes.add(create_lowpoly_deer_mesh()),
            boar: meshes.add(create_lowpoly_boar_mesh()),
            goblin: meshes.add(create_lowpoly_goblin_mesh()),
            peasant: meshes.add(create_lowpoly_peasant_mesh()),
            pet: meshes.add(create_lowpoly_pet_mesh()),
            dummy: meshes.add(create_lowpoly_dummy_mesh()),
        }
    }
}

/// Spawns a fully assembled 3D creature / NPC entity with its visual PBR mesh, physics collider, and selection ring.
pub fn spawn_creature_visual_entity(
    commands: &mut Commands,
    cache: &CachedCreatureMeshes,
    materials: &mut Assets<StandardMaterial>,
    meshes: &mut Assets<Mesh>,
    id: u64,
    db_t: &crate::module_bindings::Transform,
    is_peasant: bool,
    is_pet: bool,
    npc_brain: Option<&crate::module_bindings::NpcBrain>,
) -> Entity {
    let mut visual_transform = Transform::from_xyz(0.0, -1.05, 0.0);

    let (mesh_handle, root_collider) = if is_pet {
        visual_transform.translation.y = -0.45;
        (cache.pet.clone(), Collider::cuboid(0.5, 0.8, 0.9))
    } else if let Some(ref brain) = npc_brain {
        let m = match brain.ai_type {
            crate::module_bindings::AiType::Boar => {
                visual_transform.translation.y = -1.05;
                (cache.boar.clone(), Collider::cuboid(0.8, 0.8, 1.4))
            }
            crate::module_bindings::AiType::Deer => {
                visual_transform.translation.y = -1.05;
                (cache.deer.clone(), Collider::cuboid(0.6, 1.8, 1.2))
            }
            crate::module_bindings::AiType::Goblin => {
                visual_transform.translation.y = -1.05;
                (cache.goblin.clone(), Collider::capsule(0.4, 1.3))
            }
            crate::module_bindings::AiType::Friendly | crate::module_bindings::AiType::Peasant => {
                visual_transform.translation.y = -1.05;
                (cache.peasant.clone(), Collider::capsule(0.4, 1.8))
            }
        };

        if brain.state == crate::module_bindings::BrainState::Corpse {
            visual_transform.translation.y = -0.35;
            visual_transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        }
        m
    } else if is_peasant {
        visual_transform.translation.y = -1.05;
        (cache.peasant.clone(), Collider::capsule(0.4, 1.8))
    } else {
        visual_transform.translation.y = -1.05;
        (cache.peasant.clone(), Collider::capsule(0.4, 1.8))
    };

    let npc_type_name = if is_pet {
        "Pet"
    } else if let Some(ref brain) = npc_brain {
        match brain.ai_type {
            crate::module_bindings::AiType::Boar => "Boar",
            crate::module_bindings::AiType::Deer => "Deer",
            crate::module_bindings::AiType::Goblin => "Goblin",
            crate::module_bindings::AiType::Friendly | crate::module_bindings::AiType::Peasant => "Peasant",
        }
    } else if is_peasant {
        "Peasant"
    } else {
        "NPC"
    };

    let mut entity_cmds = commands.spawn((
        Name::new(format!("NPC_{}_{}", npc_type_name, id)),
        StateScoped(GameState::InGame),
        NetworkEntity(id),
        SpatialBundle::from_transform(Transform::from_xyz(db_t.x, db_t.y, db_t.z)),
        LogicalPosition(Vec3::new(db_t.x, db_t.y, db_t.z)),
        LogicalRotation(Quat::IDENTITY),
        Selectable, 
        RigidBody::Kinematic, 
        root_collider,
        CollisionLayers::new([GameLayer::Unit], [GameLayer::Default, GameLayer::Environment, GameLayer::Glass]),
    ));

    if is_peasant {
        entity_cmds.insert(PeasantUnit { entity_id: id });
    }

    entity_cmds.with_children(|parent| {
        parent.spawn((
            PbrBundle {
                mesh: mesh_handle,
                material: materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    perceptual_roughness: 0.85,
                    ..default()
                }),
                transform: visual_transform,
                ..default()
            },
            RenderLayers::from_layers(&[0, 1, 2]), RTSProxy,
        ));
        parent.spawn((
            PbrBundle {
                mesh: meshes.add(bevy::math::primitives::Torus::new(0.6, 0.05)),
                material: materials.add(StandardMaterial { base_color: Color::srgb(0.0, 1.0, 0.0), unlit: true, ..default() }),
                transform: Transform::from_xyz(0.0, -0.4, 0.0), visibility: Visibility::Hidden, ..default()
            },
            RenderLayers::layer(2), SelectionRing,
            NotShadowCaster,
        ));
    });

    entity_cmds.id()
}

// ----------------------------------------------------------------------------
// 6. SPARRING YARD ARENA (SANDBOX PRACTICE COMBAT SYSTEMS)
// ----------------------------------------------------------------------------

pub fn setup_sparring_yard(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    sparring_mode: Res<SparringMode>,
) {
    if !sparring_mode.0 {
        return;
    }
    info!("🥋 Sparring Yard initialized! Spawning Martial Arts Training Dummy & Sparring Goblin Raider.");

    let spawn_x = 0.0;
    let spawn_z = 0.0;
    let terrain_y = crate::terrain::get_terrain_height(spawn_x, spawn_z);

    // 1. Spawning the Martial Arts Training Dummy (3.2m directly in front of spawn)
    let dummy_pos = Vec3::new(spawn_x, terrain_y, spawn_z - 3.2);
    let dummy_mesh = meshes.add(create_lowpoly_dummy_mesh());
    let dummy_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        ..default()
    });

    commands.spawn((
        Name::new("SparringYard_TrainingDummy"),
        StateScoped(GameState::InGame),
        PbrBundle {
            mesh: dummy_mesh,
            material: dummy_mat,
            transform: BevyTransform::from_translation(dummy_pos),
            ..default()
        },
        TrainingDummy {
            wobble_timer: Timer::from_seconds(0.85, TimerMode::Once),
            wobble_angle: 0.0,
            wobble_axis: Vec3::X,
            base_rotation: Quat::IDENTITY,
        },
        RigidBody::Kinematic,
        Collider::capsule(0.35, 1.6),
        CollisionLayers::new(
            [GameLayer::Unit],
            [GameLayer::Default, GameLayer::Environment, GameLayer::Terrain],
        ),
    ));

    // 2. Spawning the Sparring Goblin Raider (3.0m forward-right, facing player)
    let goblin_pos = Vec3::new(spawn_x + 2.8, terrain_y, spawn_z - 2.6);
    let goblin_mesh = meshes.add(create_lowpoly_goblin_mesh());
    let goblin_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        ..default()
    });

    let to_player = Vec3::new(spawn_x - goblin_pos.x, 0.0, spawn_z - goblin_pos.z).normalize_or_zero();
    let goblin_rot = Quat::from_rotation_arc(Vec3::NEG_Z, to_player);

    commands.spawn((
        Name::new("SparringYard_GoblinRaider"),
        StateScoped(GameState::InGame),
        PbrBundle {
            mesh: goblin_mesh,
            material: goblin_mat,
            transform: BevyTransform::from_translation(goblin_pos).with_rotation(goblin_rot),
            ..default()
        },
        SparringGoblin {
            health: 120.0,
            max_health: 120.0,
            is_blocking: false,
            block_timer: Timer::from_seconds(0.0, TimerMode::Once),
            stagger_timer: Timer::from_seconds(0.0, TimerMode::Once),
            attack_cooldown: Timer::from_seconds(3.5, TimerMode::Repeating),
            home_pos: goblin_pos,
        },
        RigidBody::Dynamic,
        Collider::capsule(0.35, 1.2),
        LockedAxes::ROTATION_LOCKED,
        LinearVelocity::ZERO,
        CollisionLayers::new(
            [GameLayer::Unit],
            [GameLayer::Default, GameLayer::Terrain, GameLayer::Environment],
        ),
    ));

    // 3. Sparring Yard Training Torches / Ring Markers
    let ring_offsets = [
        Vec3::new(-2.8, 0.0, -5.2),
        Vec3::new(4.2, 0.0, -5.2),
        Vec3::new(-2.8, 0.0, 1.2),
        Vec3::new(4.2, 0.0, 1.2),
    ];

    let post_mesh = meshes.add(bevy::math::primitives::Cylinder::new(0.08, 1.4));
    let post_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.22, 0.12),
        perceptual_roughness: 0.9,
        ..default()
    });

    let flame_mesh = meshes.add(bevy::math::primitives::Sphere::new(0.12));
    let flame_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.65, 0.20),
        emissive: Color::srgb(2.5, 1.2, 0.3).into(),
        unlit: true,
        ..default()
    });

    for offset in ring_offsets {
        let post_y = crate::terrain::get_terrain_height(offset.x, offset.z);
        commands.spawn((
            PbrBundle {
                mesh: post_mesh.clone(),
                material: post_mat.clone(),
                transform: BevyTransform::from_xyz(offset.x, post_y + 0.7, offset.z),
                ..default()
            },
        )).with_children(|builder| {
            builder.spawn(PbrBundle {
                mesh: flame_mesh.clone(),
                material: flame_mat.clone(),
                transform: BevyTransform::from_xyz(0.0, 0.75, 0.0),
                ..default()
            });
            builder.spawn(PointLightBundle {
                point_light: PointLight {
                    color: Color::srgb(1.0, 0.75, 0.4),
                    intensity: 3500.0,
                    range: 8.0,
                    shadows_enabled: false,
                    ..default()
                },
                transform: BevyTransform::from_xyz(0.0, 0.85, 0.0),
                ..default()
            });
        });
    }
}

pub fn update_training_dummy_wobble(
    time: Res<Time>,
    mut dummy_q: Query<(&mut TrainingDummy, &mut BevyTransform)>,
) {
    for (mut dummy, mut transform) in dummy_q.iter_mut() {
        if !dummy.wobble_timer.finished() {
            dummy.wobble_timer.tick(time.delta());
            let t = dummy.wobble_timer.fraction();
            // Damped harmonic oscillation: decaying sine wave
            let decay = (-t * 4.5).exp();
            let wave = (t * 18.0).sin();
            let angle = dummy.wobble_angle * decay * wave;

            let wobble_rot = Quat::from_axis_angle(dummy.wobble_axis, angle);
            transform.rotation = dummy.base_rotation * wobble_rot;
        } else {
            transform.rotation = dummy.base_rotation;
        }
    }
}

pub fn update_sparring_goblin_ai(
    time: Res<Time>,
    mut commands: Commands,
    player_q: Query<(Entity, &BevyTransform, &LinearVelocity), (With<PlayerBody>, Without<SparringGoblin>)>,
    mut goblin_q: Query<(Entity, &mut SparringGoblin, &mut BevyTransform, &mut LinearVelocity), Without<PlayerBody>>,
    audio_handles: Option<Res<CombatAudioHandles>>,
    swing_state: Res<SwingState>,
) {
    let Ok((_, player_transform, _)) = player_q.get_single() else { return; };
    let player_pos = player_transform.translation;

    for (_, mut goblin, mut goblin_transform, mut goblin_vel) in goblin_q.iter_mut() {
        goblin.stagger_timer.tick(time.delta());
        goblin.block_timer.tick(time.delta());
        goblin.attack_cooldown.tick(time.delta());

        let to_player = player_pos - goblin_transform.translation;
        let dist = to_player.length();

        // Always face player if within 12m
        if dist < 12.0 && dist > 0.1 {
            let look_target = Vec3::new(player_pos.x, goblin_transform.translation.y, player_pos.z);
            goblin_transform.look_at(look_target, Vec3::Y);
        }

        // If staggered from a heavy blow, slow down and wait
        if !goblin.stagger_timer.finished() {
            goblin_vel.x *= 0.85;
            goblin_vel.z *= 0.85;
            continue;
        }

        // Reactive Block AI:
        // If player is winding up a melee attack within reach (dist <= 3.8m), raise block guard!
        if swing_state.phase == MeleeAttackPhase::Windup && dist <= 3.8 {
            goblin.is_blocking = true;
            goblin.block_timer.set_duration(std::time::Duration::from_secs_f32(0.45));
            goblin.block_timer.reset();
        } else if goblin.block_timer.finished() {
            goblin.is_blocking = false;
        }

        // Sparring attack cadence:
        // When within 2.5m, goblin attempts a telegraphed sparring strike every ~3.5s
        if dist <= 2.6 && goblin.attack_cooldown.just_finished() && !goblin.is_blocking {
            let strike_pos = goblin_transform.translation + goblin_transform.forward().as_vec3() * 1.2 + Vec3::Y * 1.0;

            if swing_state.is_blocking {
                // PLAYER SUCCESSFULLY BLOCKED WITH SHIELD / WEAPON!
                if let Some(ref handles) = audio_handles {
                    play_sound(&mut commands, &handles.shield_block);
                }
                // Stagger goblin back from the solid shield block
                goblin.stagger_timer = Timer::from_seconds(0.55, TimerMode::Once);
                goblin_vel.x = -to_player.x.signum() * 3.5;
                goblin_vel.z = -to_player.z.signum() * 3.5;
                spawn_comic_damage_floater(&mut commands, strike_pos, "PARRIED! CLANG", true);
            } else {
                // Goblin hits player (sparring tap)
                if let Some(ref handles) = audio_handles {
                    play_sound(&mut commands, &handles.flesh_impact);
                }
                spawn_comic_damage_floater(&mut commands, strike_pos, "BLOCKED BY NONE -14", false);
            }
        }

        // Maintain sparring distance (around 2.2m - 3.0m)
        if dist > 3.2 && dist < 12.0 {
            let forward = goblin_transform.forward().as_vec3();
            goblin_vel.x = forward.x * 2.2;
            goblin_vel.z = forward.z * 2.2;
        } else if dist < 1.8 {
            let back = -goblin_transform.forward().as_vec3();
            goblin_vel.x = back.x * 1.5;
            goblin_vel.z = back.z * 1.5;
        } else {
            goblin_vel.x *= 0.85;
            goblin_vel.z *= 0.85;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_creature_meshes_initialization() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Assets<Mesh>>();

        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        let cache = CachedCreatureMeshes::new(&mut meshes);

        assert!(meshes.get(&cache.deer).is_some());
        assert!(meshes.get(&cache.boar).is_some());
        assert!(meshes.get(&cache.goblin).is_some());
        assert!(meshes.get(&cache.peasant).is_some());
        assert!(meshes.get(&cache.pet).is_some());
        assert!(meshes.get(&cache.dummy).is_some());

        // Verify low-poly vertex budgets (each creature between 40 and 1200 vertices / <400 triangles, ~95% fewer than micro-voxels)
        let deer_mesh = meshes.get(&cache.deer).unwrap();
        assert!(deer_mesh.count_vertices() > 40 && deer_mesh.count_vertices() < 1200, "Deer verts: {}", deer_mesh.count_vertices());

        let boar_mesh = meshes.get(&cache.boar).unwrap();
        assert!(boar_mesh.count_vertices() > 40 && boar_mesh.count_vertices() < 1200, "Boar verts: {}", boar_mesh.count_vertices());

        let goblin_mesh = meshes.get(&cache.goblin).unwrap();
        assert!(goblin_mesh.count_vertices() > 40 && goblin_mesh.count_vertices() < 1200, "Goblin verts: {}", goblin_mesh.count_vertices());

        let peasant_mesh = meshes.get(&cache.peasant).unwrap();
        assert!(peasant_mesh.count_vertices() > 40 && peasant_mesh.count_vertices() < 1200, "Peasant verts: {}", peasant_mesh.count_vertices());

        let pet_mesh = meshes.get(&cache.pet).unwrap();
        assert!(pet_mesh.count_vertices() > 40 && pet_mesh.count_vertices() < 800, "Pet verts: {}", pet_mesh.count_vertices());

        let dummy_mesh = meshes.get(&cache.dummy).unwrap();
        assert!(dummy_mesh.count_vertices() > 40 && dummy_mesh.count_vertices() < 1200, "Dummy verts: {}", dummy_mesh.count_vertices());
    }
}
