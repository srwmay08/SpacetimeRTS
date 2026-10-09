// ============================================================================
// File: client/src/ui/mod.rs
// ============================================================================
// ----------------------------------------------------------------------------
// MODULAR CLIENT UI ARCHITECTURE
// ----------------------------------------------------------------------------

pub mod types;
pub mod setup;
pub mod hud;
pub mod inventory;
pub mod crafting;
pub mod console;
pub mod combat_feedback;
pub mod skills;
pub mod options;
pub mod spellbook;
pub mod hotbar;

pub use types::*;
pub use setup::*;
pub use hud::*;
pub use inventory::*;
pub use crafting::*;
pub use console::*;
pub use combat_feedback::*;
pub use skills::*;
