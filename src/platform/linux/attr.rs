// Copyright 2021 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::object_store::Timestamp;
use fuse3::{raw::prelude::FileAttr, FileType};
use std::time::SystemTime;

pub fn create_dir_attr(
    id: u64,
    size: u64,
    creation_time: Timestamp,
    modification_time: Timestamp,
    mode: u32,
) -> FileAttr {
    FileAttr {
        ino: id,
        generation: 0,
        size,
        blocks: 0,
        atime: SystemTime::UNIX_EPOCH.into(),
        mtime: modification_time.into(),
        ctime: creation_time.into(),
        kind: FileType::Directory,
        perm: fuse3::perm_from_mode_and_kind(FileType::Directory, mode),
        nlink: 0,
        uid: 0,
        gid: 0,
        rdev: 0,
        blksize: 0,
    }
}

pub fn create_file_attr(
    id: u64,
    size: u64,
    creation_time: Timestamp,
    modification_time: Timestamp,
    mode: u32,
) -> FileAttr {
    FileAttr {
        ino: id,
        generation: 0,
        size,
        blocks: 0,
        atime: SystemTime::UNIX_EPOCH.into(),
        mtime: modification_time.into(),
        ctime: creation_time.into(),
        kind: FileType::RegularFile,
        perm: fuse3::perm_from_mode_and_kind(FileType::RegularFile, mode),
        nlink: 0,
        uid: 0,
        gid: 0,
        rdev: 0,
        blksize: 0,
    }
}

impl From<Timestamp> for fuse3::Timestamp {
    fn from(time: Timestamp) -> fuse3::Timestamp {
        fuse3::Timestamp::new(time.secs as i64, time.nanos)
    }
}
