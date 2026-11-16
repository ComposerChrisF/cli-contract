//! The guarded write when the write itself fails.  It runs in a process of its own, since it
//! lowers the process's file-size limit, which would fail other tests' writes too.
//!
//! rustdoc of write_file: under `Allow`, "the old file survives a failed write"; under `Deny`,
//! "A write that fails removes the file it made, while that file is still the one at the path";
//! and of the temporary file, "After any error it is removed, if it can be".
#![cfg(unix)]

use cli_contract::{write_file, Clobber, PathError};

#[test]
fn a_failed_write_keeps_the_old_file_and_leaves_nothing_else() {
    let d = tempfile::tempdir().unwrap();
    let old = d.path().join("old.txt");
    std::fs::write(&old, b"old").unwrap();
    let new = d.path().join("new.txt");
    let too_big = [b'x'; 64];

    // A write past 8 bytes now fails with EFBIG, after writing the first 8, and SIGXFSZ, which
    // would end the process instead, is ignored.
    // SAFETY: signal and the rlimit calls change only process attributes, and `limit` outlives
    // the calls that read and write it.
    let limit = unsafe {
        assert_ne!(libc::signal(libc::SIGXFSZ, libc::SIG_IGN), libc::SIG_ERR);
        let mut limit = std::mem::zeroed::<libc::rlimit>();
        assert_eq!(libc::getrlimit(libc::RLIMIT_FSIZE, &mut limit), 0);
        let lowered = libc::rlimit {
            rlim_cur: 8,
            ..limit
        };
        assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE, &lowered), 0);
        limit
    };
    let allow = write_file(&old, &too_big, Clobber::Allow);
    let deny = write_file(&new, &too_big, Clobber::Deny);
    // SAFETY: as above.
    assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &limit) }, 0);

    assert!(
        matches!(&allow, Err(PathError::Io { path, .. }) if path == &old),
        "{allow:?}"
    );
    assert_eq!(std::fs::read(&old).unwrap(), b"old");
    assert!(
        matches!(&deny, Err(PathError::Io { path, .. }) if path == &new),
        "{deny:?}"
    );
    let mut names: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["old.txt"]);
}
