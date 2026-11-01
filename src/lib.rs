#![doc = include_str!("../README.md")]

pub mod exit;
pub mod paths;
pub mod pipe;

pub use exit::{FINDINGS, OK, SETUP, TOOL_ERROR, USAGE};
pub use paths::{
    check_destination, check_output_file, ensure_destination, is_mount_root, prepare_output_file,
    reject_symlink, require_input_dir, require_input_path, require_marker, Clobber, CreateMode,
    DestPolicy, MarkerName, MarkerNameError, ParseValueError, PathError, PolicyError,
};
pub use pipe::{ignore_broken_pipe, is_broken_pipe, reset_sigpipe};
