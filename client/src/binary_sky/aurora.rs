// ============================================================================
// File: client/src/binary_sky/aurora.rs
// ============================================================================
// ----------------------------------------------------------------------------
// CUSTOM WGSL AURORA BOREALIS MATERIAL & SKY DOME COMPONENTS
// ----------------------------------------------------------------------------

#![allow(dead_code)]

use bevy::prelude::*;
use bevy::pbr::Material;
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};

/// Uniform buffer passed to the procedural aurora sky dome WGSL shader.
#[allow(dead_code)]
#[derive(Clone, Copy, ShaderType, Debug, Reflect)]
pub struct SkyUniforms {
    pub night_factor: f32,
    pub weather_intensity: f32,
    pub speed: f32,
    pub brightness: f32,
}

impl Default for SkyUniforms {
    fn default() -> Self {
        Self {
            night_factor: 0.0,
            weather_intensity: 0.0,
            speed: 1.0,
            brightness: 1.0,
        }
    }
}

/// Custom Bevy PBR Material rendering animated planetary solar wind auroras across the sky dome.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct StarAuroraDomeMaterial {
    #[uniform(0)]
    pub uniforms: SkyUniforms,
}

impl Default for StarAuroraDomeMaterial {
    fn default() -> Self {
        Self {
            uniforms: SkyUniforms::default(),
        }
    }
}

impl Material for StarAuroraDomeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/aurora.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline<Self>,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        if let Some(ref mut depth_stencil) = descriptor.depth_stencil {
            depth_stencil.depth_write_enabled = false;
            depth_stencil.depth_compare = bevy::render::render_resource::CompareFunction::GreaterEqual;
        }
        Ok(())
    }
}

/// Component tag for the procedural planetary aurora sky dome.
#[derive(Component, Debug, Default)]
pub struct StarAuroraDome;

/// Component tag for the procedural atmospheric sky dome rendering Rayleigh/Mie scattering.
#[derive(Component, Debug, Default)]
pub struct AtmosphericSkyDome;

/// Component tag for the dynamic volumetric rain precipitation streaks.
#[derive(Component, Debug, Default)]
pub struct PrecipitationStreaks;
