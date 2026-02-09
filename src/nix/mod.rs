//! Nix interaction layer
//!
//! Handles all interactions with NixOS and Home-Manager:
//! - System detection (Flakes vs Channels, HM standalone vs module)
//! - Generation listing and parsing
//! - Package extraction
//! - Command execution (restore, delete)

pub mod commands;
pub mod detect;
pub mod generations;
pub mod packages;
pub mod services;
pub mod storage;
pub mod sysinfo;

pub use commands::{delete_generations, restore_generation, CommandResult};
pub use detect::detect_system;
pub use generations::{list_generations, GenerationSource};
pub use packages::get_packages;
