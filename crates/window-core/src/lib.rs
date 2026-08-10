pub mod sniff;
pub mod watch;

pub use sniff::{extension_lower, file_name, has_extension, is_gzip};
pub use watch::{watch_file, FileWatch};
