//! The logic of the Pleasure Horizons Pack Creator, with no window in it: what a project is, where a pack's files go, what the game says it accepts, and the checks
//! that find what is wrong with a pack. The application (`src-tauri`) only carries these to the screen.

pub mod arm_fix;
pub mod check;
pub mod convert;
pub mod diagnostics;
pub mod error;
pub mod export;
pub mod fapcraft;
pub mod fs_ops;
pub mod gamelog;
pub mod history;
pub mod import;
pub mod json_ast;
pub mod layout;
pub mod oldpack;
pub mod project;
pub mod recent;
pub mod reference;
pub mod scope;
pub mod review;
pub mod schema;
pub mod skeleton_ids;
pub mod social;
#[cfg(test)]
mod test_pack;
pub mod strut_anim;
pub mod strut_model;
pub mod templates;

pub use error::{Error, Result};
