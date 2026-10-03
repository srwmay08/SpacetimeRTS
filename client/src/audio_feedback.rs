// ============================================================================
// File: client/src/audio_feedback.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PROCEDURAL AUDIO FEEDBACK & ACOUSTIC COMBAT SENSORY SYSTEM
// ----------------------------------------------------------------------------
// Architectural Note: Implements an auditory-first combat feedback philosophy.
// Generates authentic, high-impact in-memory 16-bit PCM WAV audio cues:
// - Sharp "Dink": Crystal-clear ~2200 Hz harmonic metallic chime for critical/headshot hits.
// - Armor Shatter: Downward FM sweep with noise texture for armor breaks & absorption.
// - Bodyshot Tick: Minimalist, low-intrusion 950 Hz click for confirmed standard hits.
// - Phase Dash Whoosh: Aerodynamic air displacement for tactical movement.
// - Sonar Ping: High-resonance 1750 Hz ping for reconnaissance pulses.
// - Smoke Hiss: Pressurized canister release for space-creating smoke veils.

use bevy::prelude::*;
use std::f32::consts::PI;
use crate::components::CombatAudioHandles;

/// Architectural Note: Synthesizes a valid 16-bit Mono PCM WAV byte vector from raw floating-point samples.
pub fn create_pcm_wav(sample_rate: u32, samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(44 + samples.len() * 2);
    
    // RIFF Header
    bytes.extend_from_slice(b"RIFF");
    let file_size: u32 = 36 + (samples.len() * 2) as u32;
    bytes.extend_from_slice(&file_size.to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    
    // Format Subchunk
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (16 for PCM)
    bytes.extend_from_slice(&1u16.to_le_bytes());  // AudioFormat (1 = PCM)
    bytes.extend_from_slice(&1u16.to_le_bytes());  // NumChannels (1 = Mono)
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * 2;
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());  // BlockAlign
    bytes.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample
    
    // Data Subchunk
    bytes.extend_from_slice(b"data");
    let data_size = (samples.len() * 2) as u32;
    bytes.extend_from_slice(&data_size.to_le_bytes());
    
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let val = (clamped * 32767.0) as i16;
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

/// Synthesizes the satisfying, crisp headshot "Dink" (2200 Hz with bright 4400 Hz & 6600 Hz overtones).
pub fn generate_sharp_dink_wav() -> Vec<u8> {
    let sample_rate = 44100;
    let duration = 0.085; // 85 ms
    let total_samples = (sample_rate as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(total_samples);

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let progress = t / duration;
        let env = (-progress * 11.0).exp();

        let f1 = (2.0 * PI * 2200.0 * t).sin();
        let f2 = 0.45 * (2.0 * PI * 4400.0 * t).sin();
        let f3 = 0.22 * (2.0 * PI * 6600.0 * t).sin();
        let sample = (f1 + f2 + f3) * env * 0.75;
        samples.push(sample);
    }
    create_pcm_wav(sample_rate, &samples)
}

/// Synthesizes the glass-shatter / armor plate crunch sound.
pub fn generate_armor_break_wav() -> Vec<u8> {
    let sample_rate = 44100;
    let duration = 0.13; // 130 ms
    let total_samples = (sample_rate as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let mut lfsr: u32 = 0xACE1;

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let progress = t / duration;
        let env = (-progress * 8.5).exp();

        // Downward sweep 1500 Hz -> 450 Hz
        let freq = 1500.0 - 1050.0 * progress;
        let tone = (2.0 * PI * freq * t).sin();

        // White noise hash
        lfsr = (lfsr >> 1) ^ (-( (lfsr & 1) as i32 ) as u32 & 0xB400);
        let noise = ((lfsr & 0xFFFF) as f32 / 32768.0) - 1.0;

        let sample = (tone * 0.45 + noise * 0.55) * env * 0.8;
        samples.push(sample);
    }
    create_pcm_wav(sample_rate, &samples)
}

/// Synthesizes a subtle, non-intrusive 950 Hz hit confirmation click (35 ms).
pub fn generate_bodyshot_tick_wav() -> Vec<u8> {
    let sample_rate = 44100;
    let duration = 0.038; // 38 ms
    let total_samples = (sample_rate as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(total_samples);

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let progress = t / duration;
        let env = (-progress * 18.0).exp();
        let tone = (2.0 * PI * 950.0 * t).sin();
        let sample = tone * env * 0.55;
        samples.push(sample);
    }
    create_pcm_wav(sample_rate, &samples)
}

/// Synthesizes a high-speed aerodynamic dash swoosh.
pub fn generate_dash_whoosh_wav() -> Vec<u8> {
    let sample_rate = 44100;
    let duration = 0.22;
    let total_samples = (sample_rate as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let mut lfsr: u32 = 0x5432;

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let progress = t / duration;
        let env = (progress * PI).sin(); // Smooth rise and fall

        let freq = 420.0 - 200.0 * progress;
        let tone = (2.0 * PI * freq * t).sin();

        lfsr = (lfsr >> 1) ^ (-( (lfsr & 1) as i32 ) as u32 & 0xB400);
        let noise = ((lfsr & 0xFFFF) as f32 / 32768.0) - 1.0;

        let sample = (tone * 0.3 + noise * 0.7) * env * 0.65;
        samples.push(sample);
    }
    create_pcm_wav(sample_rate, &samples)
}

/// Synthesizes an acoustic sonar chime for Intel Dart pulses (1750 Hz resonant ping).
pub fn generate_sonar_ping_wav() -> Vec<u8> {
    let sample_rate = 44100;
    let duration = 0.26;
    let total_samples = (sample_rate as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(total_samples);

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let progress = t / duration;
        let env = (-progress * 6.5).exp();

        let fundamental = (2.0 * PI * 1750.0 * t).sin();
        let harmonic = 0.35 * (2.0 * PI * 3500.0 * t).sin();
        let sample = (fundamental + harmonic) * env * 0.70;
        samples.push(sample);
    }
    create_pcm_wav(sample_rate, &samples)
}

/// Synthesizes a pressurized canister hiss for Smoke Veil deployment.
pub fn generate_smoke_hiss_wav() -> Vec<u8> {
    let sample_rate = 44100;
    let duration = 0.32;
    let total_samples = (sample_rate as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let mut lfsr: u32 = 0x1234;

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let progress = t / duration;
        let env = (-progress * 5.0).exp();

        lfsr = (lfsr >> 1) ^ (-( (lfsr & 1) as i32 ) as u32 & 0xB400);
        let noise = ((lfsr & 0xFFFF) as f32 / 32768.0) - 1.0;
        let sample = noise * env * 0.60;
        samples.push(sample);
    }
    create_pcm_wav(sample_rate, &samples)
}

/// Startup system initializing all procedural combat sound effects.
pub fn setup_procedural_combat_audio(
    mut commands: Commands,
    mut audio_assets: ResMut<Assets<AudioSource>>,
) {
    let dink = audio_assets.add(AudioSource { bytes: generate_sharp_dink_wav().into() });
    let armor_break = audio_assets.add(AudioSource { bytes: generate_armor_break_wav().into() });
    let bodyshot_tick = audio_assets.add(AudioSource { bytes: generate_bodyshot_tick_wav().into() });
    let dash_whoosh = audio_assets.add(AudioSource { bytes: generate_dash_whoosh_wav().into() });
    let sonar_ping = audio_assets.add(AudioSource { bytes: generate_sonar_ping_wav().into() });
    let smoke_hiss = audio_assets.add(AudioSource { bytes: generate_smoke_hiss_wav().into() });

    commands.insert_resource(CombatAudioHandles {
        dink,
        armor_break,
        bodyshot_tick,
        dash_whoosh,
        sonar_ping,
        smoke_hiss,
    });
}

/// Helper function to play a synthesized sound effect.
pub fn play_sound(commands: &mut Commands, handle: &Handle<AudioSource>) {
    commands.spawn(AudioBundle {
        source: handle.clone(),
        settings: PlaybackSettings::DESPAWN,
    });
}
