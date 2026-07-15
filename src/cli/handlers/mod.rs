mod common;

pub mod build;
pub mod clean;
pub mod config;
pub mod figure;
pub mod fmt;
pub mod init;
pub mod lib;
pub mod md2html;
pub mod md2pdf;
pub mod new;
pub mod open;
pub mod publish;
pub mod quality;
pub mod template;
pub mod theme;
pub mod update;
pub mod watch;

pub use build::handle_build;
pub use clean::handle_clean;
pub use config::handle_config;
pub use figure::{handle_figure, BitfieldOptions};
pub use fmt::handle_fmt;
pub use init::handle_init;
pub use lib::handle_lib;
pub use md2html::handle_md2html;
pub use md2pdf::handle_md2pdf;
pub use new::handle_new;
pub use open::handle_open;
pub use publish::handle_publish;
pub use quality::{
    handle_ci, handle_config_validate, handle_deps, handle_doctor, handle_lint, handle_lock,
    handle_plugin,
};
pub use template::handle_template_validate;
pub use theme::handle_theme;
pub use update::handle_update;
pub use watch::handle_watch;
