pub mod bitfield;
pub mod build;
pub mod converter;
pub mod figure;
pub mod format;

// 预留接口（第二阶段或未来）
// pub mod watch;
// pub mod typst;
// pub mod epub;

pub use build::BuildService;
pub use converter::ConverterService;
pub use figure::FigureService;
pub use format::FormatService;
