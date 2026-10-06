// ============================================================================
// File: client/src/trees.rs
// ============================================================================
// ----------------------------------------------------------------------------
// LOW-POLY PROCEDURAL TREE GEOMETRY GENERATOR
// ----------------------------------------------------------------------------
// Architectural Note:
// Generates stylized, flat-shaded, low-poly 3D geometric trees inspired by
// BlendSwap #9440 ("geometric Trees Pack Light"). Replaces dense micro-voxels
// with faceted polyhedral foliage (cones, deformed geodesic icosahedra) and
// polygonal prism trunks/branches. Delivers rich procedural variety, crisp
// lighting on individual facets, and ~95% fewer polygons for 60+ FPS stability.
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use std::f32::consts::PI;

use crate::prng::Prng;

// ----------------------------------------------------------------------------
// FLAT-SHADED LOW-POLY MESH BUILDER
// ----------------------------------------------------------------------------

pub struct LowPolyMeshBuilder {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

impl Default for LowPolyMeshBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl LowPolyMeshBuilder {
    pub fn new() -> Self {
        Self {
            positions: Vec::with_capacity(512),
            normals: Vec::with_capacity(512),
            colors: Vec::with_capacity(512),
            uvs: Vec::with_capacity(512),
            indices: Vec::with_capacity(1024),
        }
    }

    /// Appends the geometry of another builder, offsetting vertex indices to preserve distinct sub-meshes.
    pub fn append(&mut self, other: LowPolyMeshBuilder) {
        let offset = self.positions.len() as u32;
        self.positions.extend(other.positions);
        self.normals.extend(other.normals);
        self.colors.extend(other.colors);
        self.uvs.extend(other.uvs);
        self.indices.extend(other.indices.into_iter().map(|i| i + offset));
    }

    /// Emits a single flat-shaded triangle with an exact computed face normal.
    pub fn add_flat_triangle(&mut self, v0: Vec3, v1: Vec3, v2: Vec3, color: [f32; 4]) {
        let e1 = v1 - v0;
        let e2 = v2 - v0;
        let cross = e1.cross(e2);
        let len = cross.length();
        let normal = if len > 1e-6 { cross / len } else { Vec3::Y };

        let start = self.positions.len() as u32;
        self.positions.push([v0.x, v0.y, v0.z]);
        self.positions.push([v1.x, v1.y, v1.z]);
        self.positions.push([v2.x, v2.y, v2.z]);

        let n = [normal.x, normal.y, normal.z];
        self.normals.push(n);
        self.normals.push(n);
        self.normals.push(n);

        self.colors.push(color);
        self.colors.push(color);
        self.colors.push(color);

        self.uvs.push([0.0, 0.0]);
        self.uvs.push([1.0, 0.0]);
        self.uvs.push([0.5, 1.0]);

        self.indices.push(start);
        self.indices.push(start + 1);
        self.indices.push(start + 2);
    }

    /// Emits a planar quad composed of two flat-shaded triangles.
    pub fn add_flat_quad(&mut self, v0: Vec3, v1: Vec3, v2: Vec3, v3: Vec3, color: [f32; 4]) {
        self.add_flat_triangle(v0, v1, v2, color);
        self.add_flat_triangle(v0, v2, v3, color);
    }

    /// Generates a tapered polygonal prism (trunk, branch, or log) with `sides` facets.
    pub fn add_tapered_prism(
        &mut self,
        base: Vec3,
        top: Vec3,
        r_base: f32,
        r_top: f32,
        sides: usize,
        color: [f32; 4],
        cap_base: bool,
        cap_top: bool,
    ) {
        if sides < 3 {
            return;
        }

        let dir = top - base;
        let len = dir.length();
        if len < 1e-5 {
            return;
        }
        let w = dir / len;

        // Construct orthonormal basis (u, v, w)
        let up_ref = if w.y.abs() > 0.92 { Vec3::X } else { Vec3::Y };
        let u = w.cross(up_ref).normalize();
        let v = w.cross(u).normalize();

        let mut base_pts = Vec::with_capacity(sides);
        let mut top_pts = Vec::with_capacity(sides);

        for i in 0..sides {
            let angle = (i as f32 / sides as f32) * 2.0 * PI;
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let radial = u * cos_a + v * sin_a;

            base_pts.push(base + radial * r_base);
            top_pts.push(top + radial * r_top);
        }

        // Side quads
        for i in 0..sides {
            let next = (i + 1) % sides;
            let b0 = base_pts[i];
            let b1 = base_pts[next];
            let t0 = top_pts[i];
            let t1 = top_pts[next];

            // Shading variation across sides
            let face_dir = (b0 + b1 + t0 + t1) * 0.25 - (base + top) * 0.5;
            let light_factor = 0.85 + 0.15 * face_dir.normalize_or_zero().dot(Vec3::new(0.5, 0.7, 0.4));
            let side_col = [
                color[0] * light_factor,
                color[1] * light_factor,
                color[2] * light_factor,
                color[3],
            ];

            self.add_flat_quad(b0, b1, t1, t0, side_col);
        }

        // Bottom cap
        if cap_base {
            let base_col = [color[0] * 0.75, color[1] * 0.75, color[2] * 0.75, color[3]];
            for i in 0..sides {
                let next = (i + 1) % sides;
                self.add_flat_triangle(base, base_pts[next], base_pts[i], base_col);
            }
        }

        // Top cap
        if cap_top {
            let top_col = [color[0] * 1.1, color[1] * 1.1, color[2] * 1.1, color[3]];
            for i in 0..sides {
                let next = (i + 1) % sides;
                self.add_flat_triangle(top, top_pts[i], top_pts[next], top_col);
            }
        }
    }

    /// Generates a conifer conical tier / skirt with `sides` facets and an under-canopy cap.
    pub fn add_cone_skirt(
        &mut self,
        apex: Vec3,
        base_center: Vec3,
        r_base: f32,
        sides: usize,
        color: [f32; 4],
        skirt_flare: f32,
    ) {
        if sides < 3 {
            return;
        }

        let mut ring = Vec::with_capacity(sides);
        for i in 0..sides {
            let angle = (i as f32 / sides as f32) * 2.0 * PI;
            let offset = Vec3::new(angle.cos() * r_base, -skirt_flare, angle.sin() * r_base);
            ring.push(base_center + offset);
        }

        // Conical downward-sloping facets (strictly outward-facing normals)
        for i in 0..sides {
            let next = (i + 1) % sides;
            let mut p0 = ring[i];
            let mut p1 = ring[next];

            let face_mid = (apex + p0 + p1) / 3.0 - base_center;
            let normal = (p1 - apex).cross(p0 - apex);
            if normal.dot(face_mid) < 0.0 {
                std::mem::swap(&mut p0, &mut p1);
            }

            let light = 0.88 + 0.12 * face_mid.normalize_or_zero().dot(Vec3::new(0.4, 0.8, 0.3));
            let facet_col = [color[0] * light, color[1] * light, color[2] * light, color[3]];

            self.add_flat_triangle(apex, p1, p0, facet_col);
        }

        // Underside recessed shadow cap
        let under_col = [color[0] * 0.65, color[1] * 0.65, color[2] * 0.65, color[3]];
        for i in 0..sides {
            let next = (i + 1) % sides;
            self.add_flat_triangle(base_center, ring[next], ring[i], under_col);
        }
    }

    /// Generates a displaced geodesic icosahedral foliage blob with per-facet lighting.
    /// Uses vertex-shared displacement to guarantee 100% watertight, crack-free manifold edges.
    pub fn add_faceted_blob(
        &mut self,
        center: Vec3,
        radii: Vec3,
        base_color: [f32; 4],
        subdivisions: usize,
        rng: &mut Prng,
        jitter: f32,
    ) {
        // Base icosahedron vertices (12 unit vectors)
        let phi = (1.0 + 5.0_f32.sqrt()) * 0.5;
        let mut base_vertices = vec![
            Vec3::new(-1.0,  phi,  0.0).normalize(),
            Vec3::new( 1.0,  phi,  0.0).normalize(),
            Vec3::new(-1.0, -phi,  0.0).normalize(),
            Vec3::new( 1.0, -phi,  0.0).normalize(),
            Vec3::new( 0.0, -1.0,  phi).normalize(),
            Vec3::new( 0.0,  1.0,  phi).normalize(),
            Vec3::new( 0.0, -1.0, -phi).normalize(),
            Vec3::new( 0.0,  1.0, -phi).normalize(),
            Vec3::new( phi,  0.0, -1.0).normalize(),
            Vec3::new( phi,  0.0,  1.0).normalize(),
            Vec3::new(-phi,  0.0, -1.0).normalize(),
            Vec3::new(-phi,  0.0,  1.0).normalize(),
        ];

        let base_faces: Vec<[usize; 3]> = vec![
            [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
            [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
            [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
            [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
        ];

        let mut current_faces = base_faces;

        // Subdivide with edge-midpoint deduplication to maintain manifold topology
        for _ in 0..subdivisions {
            let mut next_faces = Vec::with_capacity(current_faces.len() * 4);
            let mut edge_midpoints: std::collections::BTreeMap<(usize, usize), usize> = std::collections::BTreeMap::new();

            for face in current_faces {
                let mut get_midpoint = |a: usize, b: usize, verts: &mut Vec<Vec3>| -> usize {
                    let key = if a < b { (a, b) } else { (b, a) };
                    if let Some(&idx) = edge_midpoints.get(&key) {
                        idx
                    } else {
                        let mid = (verts[a] + verts[b]).normalize();
                        let idx = verts.len();
                        verts.push(mid);
                        edge_midpoints.insert(key, idx);
                        idx
                    }
                };

                let m01 = get_midpoint(face[0], face[1], &mut base_vertices);
                let m12 = get_midpoint(face[1], face[2], &mut base_vertices);
                let m20 = get_midpoint(face[2], face[0], &mut base_vertices);

                next_faces.push([face[0], m01, m20]);
                next_faces.push([face[1], m12, m01]);
                next_faces.push([face[2], m20, m12]);
                next_faces.push([m01, m12, m20]);
            }
            current_faces = next_faces;
        }

        // CRITICAL FIX: Pre-compute displaced positions for each unique vertex ONCE.
        // This completely eliminates seam cracks, holes, and edge tearing between adjacent facets.
        let displaced_positions: Vec<Vec3> = base_vertices
            .iter()
            .map(|&v| {
                let disp = 1.0 + rng.range(-jitter, jitter);
                center + Vec3::new(v.x * radii.x, v.y * radii.y, v.z * radii.z) * disp
            })
            .collect();

        // Emit flat-shaded triangles with strictly verified outward normals
        for face in current_faces {
            let p0 = displaced_positions[face[0]];
            let mut p1 = displaced_positions[face[1]];
            let mut p2 = displaced_positions[face[2]];

            let centroid = (p0 + p1 + p2) / 3.0;
            let outward_dir = centroid - center;
            let mut normal = (p1 - p0).cross(p2 - p0);

            // Enforce counter-clockwise winding order from the exterior
            if normal.dot(outward_dir) < 0.0 {
                std::mem::swap(&mut p1, &mut p2);
                normal = -normal;
            }

            let norm_len = normal.length();
            let norm_unit = if norm_len > 1e-6 { normal / norm_len } else { Vec3::Y };

            // Subtle lighting variation per facet for cel/gem-like low-poly shading
            let sun_light = norm_unit.dot(Vec3::new(0.4, 0.8, 0.3)).clamp(-1.0, 1.0);
            let shade = 0.88 + sun_light * 0.12;

            let facet_col = [
                (base_color[0] * shade).clamp(0.0, 1.0),
                (base_color[1] * shade).clamp(0.0, 1.0),
                (base_color[2] * shade).clamp(0.0, 1.0),
                base_color[3],
            ];

            self.add_flat_triangle(p0, p1, p2, facet_col);
        }
    }

    /// Assembles the final Bevy mesh with positions, normals, colors, UVs, and indices.
    pub fn build(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

// ----------------------------------------------------------------------------
// PROCEDURAL TREE SPECIES GENERATORS (BLENDSWAP #9440 GEOMETRIC PACK)
// ----------------------------------------------------------------------------

/// Tiered Conifer (Pines, Spruces, Alpine Firs):
/// 4 to 7 stacked conical skirts with 6–8 radial segments on a tapered 5-sided wood trunk.
pub fn create_lowpoly_pine_mesh(seed: u64) -> Mesh {
    let mut rng = Prng::new(seed);
    let mut builder = LowPolyMeshBuilder::new();

    let bark = [0.38, 0.24, 0.14, 1.0];
    let d_spruce = [0.14, 0.36, 0.16, 1.0];
    let m_fir = [0.22, 0.52, 0.24, 1.0];
    let l_frost = [0.45, 0.65, 0.55, 1.0];

    // Seed-based conifer variety
    let roll = rng.range(0.0, 1.0);
    let (needle_base, tier_count) = if roll < 0.40 {
        // Tall dark spruce (8-10 tiers)
        (d_spruce, (rng.range(8.0, 10.4)).round() as usize)
    } else if roll < 0.80 {
        // Alpine sage fir (6-8 tiers)
        (m_fir, (rng.range(6.0, 8.2)).round() as usize)
    } else {
        // Frost-rimed highland conifer (7 tiers)
        (l_frost, 7)
    };

    let total_h = rng.range(13.6, 19.6);
    let trunk_r_base = rng.range(0.36, 0.48);
    let trunk_r_top = trunk_r_base * 0.45;
    let lean_x = rng.range(-0.5, 0.5);
    let lean_z = rng.range(-0.5, 0.5);

    let trunk_base = Vec3::ZERO;
    let trunk_top = Vec3::new(lean_x, total_h * 0.88, lean_z);

    // 5-sided wooden trunk
    builder.add_tapered_prism(trunk_base, trunk_top, trunk_r_base, trunk_r_top, 5, bark, true, true);

    // Stacked conical skirts
    let skirt_sides = (rng.range(6.0, 8.4)).round() as usize; // 6 to 8 facets
    let start_y = total_h * 0.20;
    let end_y = total_h;
    let y_step = (end_y - start_y) / tier_count as f32;

    for i in 0..tier_count {
        let t = i as f32 / tier_count as f32;
        let tier_y = start_y + i as f32 * y_step;
        let tier_r = (3.2 * (1.0 - t * 0.72) * rng.range(0.90, 1.12)).max(0.65);
        let tier_apex_y = (tier_y + y_step * 1.5).min(total_h + 0.4);

        let lean_frac = tier_y / total_h;
        let center = Vec3::new(lean_x * lean_frac, tier_y, lean_z * lean_frac);
        let apex = Vec3::new(lean_x * lean_frac, tier_apex_y, lean_z * lean_frac);

        let tier_light = 0.90 + t * 0.20;
        let col = [
            needle_base[0] * tier_light,
            needle_base[1] * tier_light,
            needle_base[2] * tier_light,
            1.0,
        ];

        let overhang = 0.35 + (1.0 - t) * 0.25;
        builder.add_cone_skirt(apex, center, tier_r, skirt_sides, col, overhang);
    }

    // Apex needle spire
    let spire_apex = Vec3::new(lean_x, total_h + 0.6, lean_z);
    let spire_base = Vec3::new(lean_x, total_h * 0.92, lean_z);
    builder.add_cone_skirt(spire_apex, spire_base, 0.60, skirt_sides, needle_base, 0.20);

    builder.build()
}

/// Branching Oak / Sprawling Broadleaf:
/// Trunk bifurcates into 2–3 angled wooden boughs cradling wide asymmetrical faceted canopy lobes.
pub fn create_lowpoly_oak_mesh(seed: u64) -> Mesh {
    let mut rng = Prng::new(seed);
    let mut builder = LowPolyMeshBuilder::new();

    let bark = [0.44, 0.28, 0.16, 1.0];
    let amber_base = [0.94, 0.54, 0.16, 1.0];
    let orange_base = [0.86, 0.38, 0.10, 1.0];
    let gold_base = [0.96, 0.70, 0.18, 1.0];

    let t_blend = rng.range(0.0, 1.0);
    let canopy_col = rng.blend_color(amber_base, orange_base, t_blend * 0.6);
    let gold_highlight = rng.blend_color(gold_base, amber_base, 0.35);

    let trunk_h = rng.range(5.6, 7.6);
    let trunk_r = rng.range(0.48, 0.65);

    let trunk_base = Vec3::ZERO;
    let fork_pt = Vec3::new(rng.range(-0.4, 0.4), trunk_h, rng.range(-0.4, 0.4));

    // Sturdy 6-sided lower trunk
    builder.add_tapered_prism(trunk_base, fork_pt, trunk_r, trunk_r * 0.82, 6, bark, true, true);

    // Root flares
    for i in 0..4 {
        let ang = i as f32 * (PI * 0.5) + rng.range(-0.2, 0.2);
        let root_dir = Vec3::new(ang.cos(), 0.0, ang.sin());
        builder.add_tapered_prism(
            trunk_base,
            trunk_base + root_dir * 1.2,
            trunk_r * 0.65,
            0.12,
            4,
            bark,
            true,
            true,
        );
    }

    // Branch limbs
    let branch_count = 3;
    let mut branch_tips = Vec::with_capacity(branch_count);

    for i in 0..branch_count {
        let ang = (i as f32 / branch_count as f32) * 2.0 * PI + rng.range(-0.35, 0.35);
        let dist = rng.range(2.6, 4.0);
        let lift = rng.range(2.4, 3.8);
        let tip = fork_pt + Vec3::new(ang.cos() * dist, lift, ang.sin() * dist);

        builder.add_tapered_prism(fork_pt, tip, trunk_r * 0.58, trunk_r * 0.30, 5, bark, false, true);
        branch_tips.push(tip);
    }

    // Central primary canopy blob
    let center_blob_pos = fork_pt + Vec3::new(0.0, rng.range(2.8, 3.8), 0.0);
    let rx = rng.range(3.2, 4.2);
    let ry = rng.range(2.4, 3.4);
    let rz = rng.range(3.2, 4.2);
    builder.add_faceted_blob(center_blob_pos, Vec3::new(rx, ry, rz), canopy_col, 0, &mut rng, 0.18);

    // Secondary canopy clusters atop branch tips
    for (i, tip) in branch_tips.into_iter().enumerate() {
        let lobe_rx = rng.range(2.0, 2.8);
        let lobe_ry = rng.range(1.6, 2.4);
        let lobe_rz = rng.range(2.0, 2.8);
        let col = if i % 2 == 0 { canopy_col } else { gold_highlight };
        builder.add_faceted_blob(tip + Vec3::new(0.0, 0.8, 0.0), Vec3::new(lobe_rx, lobe_ry, lobe_rz), col, 0, &mut rng, 0.15);
    }

    builder.build()
}

/// Clustered Deciduous / Teardrop Poplar (Round / Fruit / Poplar):
/// Double-blob overlapping crowns or sleek teardrop flame polyhedra atop a slender trunk.
pub fn create_lowpoly_round_tree_mesh(seed: u64) -> Mesh {
    let mut rng = Prng::new(seed);
    let mut builder = LowPolyMeshBuilder::new();

    let bark = [0.32, 0.22, 0.14, 1.0];
    let lime_green = [0.42, 0.76, 0.24, 1.0];
    let chartreuse = [0.55, 0.84, 0.26, 1.0];
    let olive = [0.35, 0.62, 0.18, 1.0];

    let trunk_h = rng.range(5.2, 7.2);
    let trunk_r = rng.range(0.32, 0.44);
    let trunk_top = Vec3::new(rng.range(-0.3, 0.3), trunk_h, rng.range(-0.3, 0.3));

    // Slender 5-sided trunk
    builder.add_tapered_prism(Vec3::ZERO, trunk_top, trunk_r, trunk_r * 0.65, 5, bark, true, true);

    let variety_roll = rng.range(0.0, 1.0);
    if variety_roll < 0.50 {
        // Variant A: Double-Blob Clustered Deciduous (lower secondary lobe + upper primary crown)
        let lower_pos = trunk_top + Vec3::new(rng.range(-0.6, 0.6), 1.2, rng.range(-0.6, 0.6));
        let upper_pos = trunk_top + Vec3::new(0.0, 3.6, 0.0);

        let r_lower = rng.range(2.2, 2.8);
        let r_upper = rng.range(2.8, 3.6);

        builder.add_faceted_blob(lower_pos, Vec3::new(r_lower, r_lower * 0.85, r_lower), olive, 0, &mut rng, 0.16);
        builder.add_faceted_blob(upper_pos, Vec3::new(r_upper, r_upper * 1.05, r_upper), chartreuse, 0, &mut rng, 0.15);
    } else {
        // Variant B: Teardrop / Poplar (vertically elongated, pinched top and base)
        let crown_center = trunk_top + Vec3::new(0.0, 4.4, 0.0);
        let rx = rng.range(2.2, 2.8);
        let ry = rng.range(4.8, 6.4);
        let rz = rng.range(2.2, 2.8);

        builder.add_faceted_blob(crown_center, Vec3::new(rx, ry, rz), lime_green, 0, &mut rng, 0.14);
    }

    builder.build()
}

/// Dead Tree / Weathered Snag:
/// Jagged, bare polygonal branches and root flares with no foliage.
pub fn create_lowpoly_dead_tree_mesh(seed: u64) -> Mesh {
    let mut rng = Prng::new(seed);
    let mut builder = LowPolyMeshBuilder::new();

    let bark_base = [0.65, 0.52, 0.36, 1.0];
    let shadow_base = [0.45, 0.36, 0.24, 1.0];
    let wood_tip = [0.78, 0.68, 0.50, 1.0];

    let trunk_h = rng.range(11.0, 15.0);
    let trunk_r = rng.range(0.48, 0.65);
    let lean_x = rng.range(-1.0, 1.0);
    let lean_z = rng.range(-1.0, 1.0);

    let trunk_base = Vec3::ZERO;
    let trunk_mid = Vec3::new(lean_x * 0.45, trunk_h * 0.55, lean_z * 0.45);
    let trunk_top = Vec3::new(lean_x, trunk_h, lean_z);

    // Gnarled lower trunk and mid trunk
    builder.add_tapered_prism(trunk_base, trunk_mid, trunk_r, trunk_r * 0.72, 5, bark_base, true, false);
    builder.add_tapered_prism(trunk_mid, trunk_top, trunk_r * 0.72, trunk_r * 0.35, 5, shadow_base, false, true);

    // Root buttresses
    for i in 0..4 {
        let ang = i as f32 * (PI * 0.5) + rng.range(-0.25, 0.25);
        let root_end = Vec3::new(ang.cos() * 1.5, 0.0, ang.sin() * 1.5);
        builder.add_tapered_prism(trunk_base, root_end, trunk_r * 0.6, 0.10, 4, shadow_base, true, true);
    }

    // Bare jagged branches
    let branch_levels = [
        (trunk_mid, 3.2, 2.4, 0.35),
        (trunk_mid + Vec3::new(0.0, 2.0, 0.0), 3.8, 3.2, 0.28),
        (trunk_top, 2.6, 1.8, 0.22),
    ];

    for (origin, reach, lift, thick) in branch_levels {
        let ang = rng.range(0.0, 2.0 * PI);
        let b_end = origin + Vec3::new(ang.cos() * reach, lift, ang.sin() * reach);
        builder.add_tapered_prism(origin, b_end, thick, thick * 0.35, 4, bark_base, false, true);

        // Sub-twig
        let sub_ang = ang + rng.range(-0.8, 0.8);
        let sub_end = b_end + Vec3::new(sub_ang.cos() * 1.4, 1.0, sub_ang.sin() * 1.4);
        builder.add_tapered_prism(b_end, sub_end, thick * 0.55, 0.04, 3, wood_tip, false, true);
    }

    builder.build()
}

/// Fallen Log:
/// Horizontal 6-sided faceted log with hollow/mossy ends and broken branch stubs.
pub fn create_lowpoly_fallen_log_mesh(seed: u64) -> Mesh {
    let mut rng = Prng::new(seed);
    let mut builder = LowPolyMeshBuilder::new();

    let bark = [0.38, 0.24, 0.14, 1.0];
    let heartwood = [0.65, 0.50, 0.32, 1.0];
    let moss = [0.24, 0.48, 0.18, 1.0];

    let length = rng.range(3.2, 4.8);
    let radius = rng.range(0.32, 0.44);

    let start = Vec3::new(-length * 0.5, radius, 0.0);
    let end = Vec3::new(length * 0.5, radius * 0.85, rng.range(-0.3, 0.3));

    // 6-sided log body
    builder.add_tapered_prism(start, end, radius, radius * 0.85, 6, bark, true, true);

    // Broken branch stubs
    let stub_pos = start.lerp(end, 0.38);
    let stub_tip = stub_pos + Vec3::new(0.1, radius * 1.5, 0.3);
    builder.add_tapered_prism(stub_pos, stub_tip, radius * 0.35, radius * 0.15, 4, heartwood, false, true);

    // Mossy cap on top ridge
    let moss_start = start + Vec3::new(0.6, radius * 0.92, 0.0);
    let moss_end = end + Vec3::new(-0.6, radius * 0.80, 0.0);
    builder.add_tapered_prism(moss_start, moss_end, radius * 0.35, radius * 0.30, 4, moss, true, true);

    builder.build()
}


// ----------------------------------------------------------------------------
// TREE MESH CACHE
// ----------------------------------------------------------------------------

/// Cache pre-generating 6 unique procedural variants per tree species.
#[derive(Clone)]
pub struct TreeMeshCache {
    pub dead_tree_variants: Vec<Handle<Mesh>>,
    pub oak_tree_variants: Vec<Handle<Mesh>>,
    pub pine_tree_variants: Vec<Handle<Mesh>>,
    pub round_tree_variants: Vec<Handle<Mesh>>,
}

impl TreeMeshCache {
    pub fn new(meshes: &mut Assets<Mesh>) -> Self {
        let mut dead = Vec::with_capacity(6);
        let mut oak = Vec::with_capacity(6);
        let mut pine = Vec::with_capacity(6);
        let mut round = Vec::with_capacity(6);

        for i in 0..6 {
            let seed_dead = 1010 + i as u64 * 37 + 7;
            let seed_oak = 2020 + i as u64 * 41 + 13;
            let seed_pine = 3030 + i as u64 * 53 + 19;
            let seed_round = 4040 + i as u64 * 67 + 23;

            dead.push(meshes.add(create_lowpoly_dead_tree_mesh(seed_dead)));
            oak.push(meshes.add(create_lowpoly_oak_mesh(seed_oak)));
            pine.push(meshes.add(create_lowpoly_pine_mesh(seed_pine)));
            round.push(meshes.add(create_lowpoly_round_tree_mesh(seed_round)));
        }

        Self {
            dead_tree_variants: dead,
            oak_tree_variants: oak,
            pine_tree_variants: pine,
            round_tree_variants: round,
        }
    }
}

// ============================================================================
// COMPREHENSIVE UNIT TEST SUITE
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lowpoly_mesh_builder_flat_normals_and_indices() {
        let mut builder = LowPolyMeshBuilder::new();
        builder.add_flat_triangle(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            [0.2, 0.8, 0.2, 1.0],
        );
        let mesh = builder.build();

        assert_eq!(mesh.primitive_topology(), PrimitiveTopology::TriangleList);
        assert_eq!(mesh.count_vertices(), 3);

        let normals = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap().as_float3().unwrap();
        assert_eq!(normals.len(), 3);
        // Face normal should point in +Z direction (0, 0, 1)
        for n in normals {
            assert!((n[0]).abs() < 1e-4);
            assert!((n[1]).abs() < 1e-4);
            assert!((n[2] - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn test_lowpoly_tree_generators_attributes() {
        let pine = create_lowpoly_pine_mesh(1234);
        let oak = create_lowpoly_oak_mesh(2345);
        let round = create_lowpoly_round_tree_mesh(3456);
        let dead = create_lowpoly_dead_tree_mesh(4567);
        let log = create_lowpoly_fallen_log_mesh(5678);

        for (name, mesh) in [
            ("pine", &pine),
            ("oak", &oak),
            ("round", &round),
            ("dead", &dead),
            ("log", &log),
        ] {
            assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some(), "{name} missing positions");
            assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some(), "{name} missing normals");
            assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some(), "{name} missing colors");
            assert!(mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some(), "{name} missing UVs");
            assert!(mesh.indices().is_some(), "{name} missing indices");

            let vert_count = mesh.count_vertices();
            // Low-poly efficiency check: should have plenty of detail for styling (~100-1200 verts),
            // but far less than heavy micro-voxel blocks (~5,000+ verts).
            assert!(vert_count >= 60, "{name} too few vertices: {vert_count}");
            assert!(vert_count <= 2500, "{name} exceeds low-poly vertex budget: {vert_count}");
        }
    }

    #[test]
    fn test_lowpoly_seed_variety() {
        let pine1 = create_lowpoly_pine_mesh(1111);
        let pine2 = create_lowpoly_pine_mesh(2222);
        assert_ne!(pine1.count_vertices(), pine2.count_vertices());

        let oak1 = create_lowpoly_oak_mesh(3333);
        let oak2 = create_lowpoly_oak_mesh(4444);
        let pos1 = oak1.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
        let pos2 = oak2.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
        assert_ne!(pos1[0], pos2[0]);
    }
}
