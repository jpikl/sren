use fs_extra::dir;
use fs_extra::error::Error;
use fs_extra::error::ErrorKind;
use fs_extra::error::Result;
use fs_extra::file;
use once_cell::sync::Lazy;
use same_file::is_same_file;
use std::fs;
use std::path::Path;

#[derive(PartialEq, Debug)]
pub enum FileType {
    File,
    Dir,
    Unknown,
}

impl From<&Path> for FileType {
    fn from(path: &Path) -> Self {
        match path.metadata() {
            Ok(metadata) => {
                if metadata.is_dir() {
                    FileType::Dir
                } else {
                    FileType::File
                }
            }
            Err(_) => FileType::Unknown,
        }
    }
}

#[derive(Clone, Copy)]
pub enum TransferMode {
    Move,
    Copy,
}

static FILE_COPY_OPTIONS: Lazy<file::CopyOptions> = Lazy::new(|| {
    let mut options = file::CopyOptions::new();
    options.overwrite = true;
    options.skip_exist = false;
    options
});

static DIR_COPY_OPTIONS: Lazy<dir::CopyOptions> = Lazy::new(|| {
    let mut options = dir::CopyOptions::new();
    options.overwrite = true;
    options.skip_exist = false;
    options.copy_inside = true;
    options.content_only = true;
    options
});

pub fn transfer(src: &Path, dst: &Path, mode: TransferMode) -> Result<()> {
    match (FileType::from(src), FileType::from(dst)) {
        (FileType::Unknown, _) => Err(Error::new(
            ErrorKind::NotFound,
            &format!(
                "Path '{}' does not exist or you don't have access",
                src.to_string_lossy()
            ),
        )),

        (FileType::File, FileType::Dir) => Err(Error::new(
            ErrorKind::Other,
            &format!(
                "Cannot to overwrite directory '{}' with file '{}'",
                dst.to_string_lossy(),
                src.to_string_lossy()
            ),
        )),

        (FileType::Dir, FileType::File) => Err(Error::new(
            ErrorKind::Other,
            &format!(
                "Cannot to overwrite file '{}' with directory '{}'",
                dst.to_string_lossy(),
                src.to_string_lossy()
            ),
        )),

        (FileType::File, dst_type) => {
            if let Some(dst_parent) = dst.parent() {
                dir::create_all(dst_parent, false)?;
            }
            match mode {
                TransferMode::Move => {
                    if fs::rename(src, dst).is_err() {
                        file::move_file(src, dst, &FILE_COPY_OPTIONS)?;
                    }
                }
                TransferMode::Copy => {
                    if dst_type == FileType::Unknown || !is_same_file(src, dst)? {
                        file::copy(src, dst, &FILE_COPY_OPTIONS)?;
                    }
                }
            }
            Ok(())
        }

        (FileType::Dir, dst_type) => {
            dir::create_all(dst, false)?;

            match mode {
                TransferMode::Move => {
                    if fs::rename(src, dst).is_err() {
                        dir::move_dir(src, dst, &DIR_COPY_OPTIONS)?;
                    }
                }
                TransferMode::Copy => {
                    if dst_type == FileType::Unknown || !is_same_file(src, dst)? {
                        dir::copy(src, dst, &DIR_COPY_OPTIONS)?;
                    }
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;
    use assert_fs::NamedTempFile;
    use assert_fs::TempDir;
    use claim::assert_err;
    use claim::assert_ok;
    use ntest::*;
    use test_case::test_case;

    #[test_case(temp_dir().path(),            FileType::Dir     ; "dir")]
    #[test_case(touch(temp_file("a")).path(), FileType::File    ; "file")]
    #[test_case(temp_file("b").path(),        FileType::Unknown ; "unknown")]
    fn file_type(path: &Path, file_type: FileType) {
        assert_eq!(FileType::from(path), file_type);
    }

    #[test]
    fn copy_options() {
        // These options have to be the same
        assert_eq!(DIR_COPY_OPTIONS.overwrite, FILE_COPY_OPTIONS.overwrite);
        assert_eq!(DIR_COPY_OPTIONS.skip_exist, FILE_COPY_OPTIONS.skip_exist);
        assert_eq!(DIR_COPY_OPTIONS.buffer_size, FILE_COPY_OPTIONS.buffer_size);
    }

    #[test]
    fn path_not_found() {
        let src_file = temp_file("a");

        let error = assert_err!(transfer(
            src_file.path(),
            &Path::new("b"),
            TransferMode::Move // Mode is irrelevant
        ));

        assert_eq!(format!("{:?}", error.kind), "NotFound");
        assert_eq!(
            error.to_string(),
            format!(
                "Path '{}' does not exist or you don't have access",
                src_file.path().to_string_lossy()
            )
        );

        src_file.assert(predicates::path::missing());
    }

    #[test]
    fn overwrite_dir_with_file() {
        let src_file = touch(temp_file("a"));
        let dst_dir = temp_dir();

        let error = assert_err!(transfer(
            src_file.path(),
            dst_dir.path(),
            TransferMode::Move // Mode is irrelevant
        ));

        assert_eq!(format!("{:?}", error.kind), "Other");
        assert_eq!(
            error.to_string(),
            format!(
                "Cannot to overwrite directory '{}' with file '{}'",
                dst_dir.path().to_string_lossy(),
                src_file.path().to_string_lossy()
            )
        );

        src_file.assert(predicates::path::is_file());
        dst_dir.assert(predicates::path::is_dir());
    }

    #[test]
    fn overwrite_file_with_dir() {
        let src_dir = temp_dir();
        let dst_file = touch(temp_file("a"));

        let error = assert_err!(transfer(
            src_dir.path(),
            dst_file.path(),
            TransferMode::Move // Mode is irrelevant
        ));

        assert_eq!(format!("{:?}", error.kind), "Other");
        assert_eq!(
            error.to_string(),
            format!(
                "Cannot to overwrite file '{}' with directory '{}'",
                dst_file.path().to_string_lossy(),
                src_dir.path().to_string_lossy()
            )
        );

        src_dir.assert(predicates::path::is_dir());
        dst_file.assert(predicates::path::is_file());
    }

    #[test]
    fn rename_file() {
        let src_file = write(temp_file("a"), "1");
        let dst_file = temp_file("b");

        assert_ok!(transfer(
            src_file.path(),
            dst_file.path(),
            TransferMode::Move
        ));

        src_file.assert(predicates::path::missing());
        dst_file.assert("1");
    }

    #[test]
    fn rename_file_to_itself() {
        let src_file = write(temp_file("a"), "1");

        assert_ok!(transfer(
            src_file.path(),
            src_file.path(),
            TransferMode::Move
        ));

        src_file.assert("1");
    }

    #[test]
    fn move_file_to_other() {
        let src_file = write(temp_file("a"), "1");
        let dst_file = write(temp_file("b"), "2");

        assert_ok!(transfer(
            src_file.path(),
            dst_file.path(),
            TransferMode::Move
        ));

        src_file.assert(predicates::path::missing());
        dst_file.assert("1");
    }

    #[test]
    fn copy_file() {
        let src_file = write(temp_file("a"), "1");
        let dst_file = temp_file("b");

        assert_ok!(transfer(
            src_file.path(),
            dst_file.path(),
            TransferMode::Copy
        ));

        src_file.assert("1");
        dst_file.assert("1");
    }

    // This tests that we do not call fs_extra::file::copy with the same
    // source and destination path (in which case fs_extra freezes).
    // Timeout is to ensure the running test does not hang forever.
    #[test]
    #[timeout(5000)]
    fn copy_file_to_itself() {
        let src_file = write(temp_file("a"), "1");

        assert_ok!(transfer(
            src_file.path(),
            src_file.path(),
            TransferMode::Copy
        ));

        src_file.assert("1");
    }

    #[test]
    fn copy_file_to_other() {
        let src_file = write(temp_file("a"), "1");
        let dst_file = write(temp_file("b"), "2");

        assert_ok!(transfer(
            src_file.path(),
            dst_file.path(),
            TransferMode::Copy
        ));

        src_file.assert("1");
        dst_file.assert("1");
    }

    #[test]
    fn rename_dir() {
        let root_dir = temp_dir();

        let src_dir = mkdir(root_dir.child("a"));
        let src_file = write(src_dir.child("c"), "1");

        let dst_dir = root_dir.child("b");
        let dst_file = dst_dir.child("c");

        assert_ok!(transfer(src_dir.path(), dst_dir.path(), TransferMode::Move));

        src_dir.assert(predicates::path::missing());
        src_file.assert(predicates::path::missing());

        dst_dir.assert(predicates::path::is_dir());
        dst_file.assert("1");
    }

    #[test]
    fn rename_dir_to_itself() {
        let src_dir = temp_dir();
        let src_file = write(src_dir.child("a"), "1");

        assert_ok!(transfer(src_dir.path(), src_dir.path(), TransferMode::Move));

        src_dir.assert(predicates::path::is_dir());
        src_file.assert("1");
    }

    #[test]
    fn move_dir_to_other() {
        let root_dir = temp_dir();

        let src_dir = mkdir(root_dir.child("a"));
        let src_file = write(src_dir.child("c"), "1");

        let dst_dir = mkdir(root_dir.child("b"));
        let dst_file = write(dst_dir.child("c"), "2");

        assert_ok!(transfer(src_dir.path(), dst_dir.path(), TransferMode::Move));

        src_dir.assert(predicates::path::missing());
        src_file.assert(predicates::path::missing());

        dst_dir.assert(predicates::path::is_dir());
        dst_file.assert("1");
    }

    #[test]
    fn copy_dir() {
        let root_dir = temp_dir();

        let src_dir = mkdir(root_dir.child("a"));
        let src_file = write(src_dir.child("c"), "1");

        let dst_dir = root_dir.child("b");
        let dst_file = dst_dir.child("c");

        assert_ok!(transfer(src_dir.path(), dst_dir.path(), TransferMode::Copy));

        src_dir.assert(predicates::path::is_dir());
        src_file.assert("1");

        dst_dir.assert(predicates::path::is_dir());
        dst_file.assert("1");
    }

    // This tests that we do not call fs_extra::file::copy with the same
    // source and destination path (in which case fs_extra freezes).
    // Timeout is to ensure the running test does not hang forever.
    #[test]
    #[timeout(5000)]
    fn copy_dir_to_itself() {
        let src_dir = temp_dir();
        let src_file = write(src_dir.child("a"), "1");

        assert_ok!(transfer(src_dir.path(), src_dir.path(), TransferMode::Copy));

        src_dir.assert(predicates::path::is_dir());
        src_file.assert("1");
    }

    #[test]
    fn copy_dir_to_other() {
        let root_dir = temp_dir();

        let src_dir = mkdir(root_dir.child("a"));
        let src_file = write(src_dir.child("c"), "1");

        let dst_dir = mkdir(root_dir.child("b"));
        let dst_file = write(dst_dir.child("c"), "2");

        assert_ok!(transfer(src_dir.path(), dst_dir.path(), TransferMode::Copy));

        src_dir.assert(predicates::path::is_dir());
        src_file.assert("1");

        dst_dir.assert(predicates::path::is_dir());
        dst_file.assert("1");
    }

    fn temp_dir() -> TempDir {
        assert_ok!(TempDir::new())
    }

    fn temp_file(name: &str) -> NamedTempFile {
        assert_ok!(NamedTempFile::new(name))
    }

    fn mkdir<P: PathCreateDir>(path: P) -> P {
        assert_ok!(path.create_dir_all());
        path
    }

    fn touch<F: FileTouch>(file: F) -> F {
        assert_ok!(file.touch());
        file
    }

    fn write<F: FileWriteStr>(file: F, data: &str) -> F {
        assert_ok!(file.write_str(data));
        file
    }
}
