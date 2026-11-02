//! 仓储层（设计 §3.2）：settings / roots / projects / commands 各表。

pub mod commands;
pub mod projects;
pub mod roots;
pub mod runs;
pub mod settings;

pub use roots::{add_root, list_roots, remove_root, set_root_enabled, Root};
pub use commands::{
    add_command, delete_command, get_command, last_used_command, list_commands, update_command,
    CommandPatch, CommandPatchDto,
};
pub use projects::{
    command_count, delete_project, get_project, insert_command, insert_project,
    list_projects, register_project, relocate_project, search_projects, update_note,
};
pub use runs::{record_external_launch, write_back_finals, RunFinal, RunOutcome};
pub use settings::{get_settings, update_settings};
