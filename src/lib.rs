#![doc = include_str!("../README.md")]

pub mod exit;
pub mod paths;
pub mod pipe;

pub use exit::{FINDINGS, OK, SETUP, TOOL_ERROR, USAGE};
pub use paths::{
    check_destination, ensure_destination, is_mount_root, prepare_output_file, reject_symlink,
    require_input_dir, require_input_path, require_marker, symlink_info, Clobber, CreateMode,
    DestPolicy, ParseValueError, PathError, PolicyError, SymlinkInfo,
};
pub use pipe::{ignore_broken_pipe, is_broken_pipe, reset_sigpipe};
