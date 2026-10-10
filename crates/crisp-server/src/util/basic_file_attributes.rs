use std::io;
use std::path::Path;

pub struct BasicFileAttributes {
    pub symbolic_link: bool,
    pub directory: bool,
    pub regular_file: bool,
}

impl BasicFileAttributes {
    #[cfg(windows)]
    pub fn read(path: &Path) -> io::Result<BasicFileAttributes> {
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_DEVICE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FindClose, FindFirstFileW, WIN32_FIND_DATAW,
        };
        use windows_sys::Win32::System::SystemServices::IO_REPARSE_TAG_SYMLINK;

        let attributes = std::fs::symlink_metadata(path)?.file_attributes();
        let mut symbolic_link = false;
        if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let mut data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
            let handle = unsafe { FindFirstFileW(name.as_ptr(), &mut data) };
            if handle as isize == -1 {
                return Err(io::Error::last_os_error());
            }
            unsafe { FindClose(handle) };
            symbolic_link = data.dwReserved0 == IO_REPARSE_TAG_SYMLINK;
        }
        let directory = !symbolic_link && attributes & FILE_ATTRIBUTE_DIRECTORY != 0;
        let other = !symbolic_link && attributes & (FILE_ATTRIBUTE_DEVICE | FILE_ATTRIBUTE_REPARSE_POINT) != 0;
        Ok(BasicFileAttributes { symbolic_link, directory, regular_file: !symbolic_link && !directory && !other })
    }

    #[cfg(not(windows))]
    pub fn read(path: &Path) -> io::Result<BasicFileAttributes> {
        let file_type = std::fs::symlink_metadata(path)?.file_type();
        Ok(BasicFileAttributes { symbolic_link: file_type.is_symlink(), directory: file_type.is_dir(), regular_file: file_type.is_file() })
    }
}
