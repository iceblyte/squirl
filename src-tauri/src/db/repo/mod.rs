//! 仓储层（设计 §3.2）：settings / roots / projects / commands 各表。

pub mod projects;
pub mod roots;
pub mod runs;
pub mod settings;

pub use projects::{insert_command, insert_project};
pub use runs::{write_back_finals, RunFinal, RunOutcome};
pub use settings::{get_settings, update_settings};
