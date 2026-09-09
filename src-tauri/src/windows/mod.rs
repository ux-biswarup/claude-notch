//! Windows-specific behaviour (PRD §10, phases 1, 5, 6). Everything here runs
//! in user space: no services, no elevation, no machine-wide changes.

pub mod focus;
pub mod overlay;
pub mod startup;
