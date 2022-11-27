// Copyright 2021 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::ffi::{OsStr};
use std::time::{Duration};
use std::vec::IntoIter;
use async_trait::async_trait;
use fuse3::raw::prelude::*;
use fuse3::raw::prelude::Filesystem as FuseFilesystem;
use fuse3::{Result};
use futures_util::stream::{Empty, Iter};
use tracing::{Level};
use crate::filesystem::SyncOptions;
use crate::object_store::Directory;
use crate::object_store::transaction::{Options, TransactionHandler};
use crate::platform::linux::errors::cast_to_fuse_error;
use crate::platform::linux::fuse_fs::FuseFs;


const TTL: Duration = Duration::from_secs(1);
const DEFAULT_FILE_MODE: u32 = 0o755;


#[async_trait]
impl FuseFilesystem for FuseFs {
    type DirEntryStream = Empty<Result<DirectoryEntry>>;
    type DirEntryPlusStream = Iter<IntoIter<Result<DirectoryEntryPlus>>>;

    async fn init(&self, _req: Request) -> Result<()> {
        Ok(())
    }

    async fn destroy(&self, _req: Request) {
        self.fs.close().await.expect("Close failed");
    }

    async fn lookup(&self, _req: Request, parent: u64, name: &OsStr) -> Result<ReplyEntry> {
        let dir = self.open_dir(parent).await?;
        let lookup_result = dir.lookup(name.to_str().expect("Invalid UniCode file name")).await;

        if let Ok(object) = lookup_result {
            if let Some((object_id, object_descriptor)) = object {
                Ok(ReplyEntry {
                    ttl: TTL,
                    attr: self.create_object_attr(object_id, object_descriptor).await?,
                    generation: 0,
                })
            } else {
                Err(libc::ENOENT.into())
            }
        } else {
            Err(cast_to_fuse_error(&lookup_result.err().unwrap()))
        }
    }


    async fn forget(&self, _req: Request, _inode: u64, _nlookup: u64) {
        unimplemented!()
    }

    async fn getattr(
        &self,
        _req: Request,
        inode: u64,
        _fh: Option<u64>,
        _flags: u32,
    ) -> Result<ReplyAttr> {
        unimplemented!()
    }

    async fn setattr(
        &self,
        _req: Request,
        inode: u64,
        _fh: Option<u64>,
        set_attr: SetAttr,
    ) -> Result<ReplyAttr> {
        unimplemented!()
    }

    async fn mkdir(
        &self,
        _req: Request,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
    ) -> Result<ReplyEntry> {
        let mut transaction = self.fs
            .clone()
            .new_transaction(&[], Options::default())
            .await
            .expect("new_transaction failed");

        let dir = self.open_dir(parent).await?;

        let child_dir_result = dir
            .create_child_dir(&mut transaction, name.to_str().expect("Invalid UniCode file name"))
            .await;

        if let Ok(child_dir) = child_dir_result {
            transaction.commit().await.expect("commit failed");
            fs.sync(SyncOptions::default()).await.expect("sync failed");
            Err(cast_to_fuse_error(&child_dir_result.err().unwrap()))
        } else {
            Err(cast_to_fuse_error(&child_dir_result.err().unwrap()))
        }

    }

    async fn unlink(&self, _req: Request, parent: u64, name: &OsStr) -> Result<()> {
        unimplemented!()
    }

    async fn rmdir(&self, _req: Request, parent: u64, name: &OsStr) -> Result<()> {
        unimplemented!()
    }

    async fn rename(
        &self,
        _req: Request,
        parent: u64,
        name: &OsStr,
        new_parent: u64,
        new_name: &OsStr,
    ) -> Result<()> {
        unimplemented!()
    }

    async fn open(&self, _req: Request, inode: u64, _flags: u32) -> Result<ReplyOpen> {
        unimplemented!()
    }

    async fn read(
        &self,
        _req: Request,
        inode: u64,
        _fh: u64,
        offset: u64,
        size: u32,
    ) -> Result<ReplyData> {
        unimplemented!()
    }

    async fn write(
        &self,
        _req: Request,
        inode: u64,
        _fh: u64,
        offset: u64,
        mut data: &[u8],
        _flags: u32,
    ) -> Result<ReplyWrite> {
        unimplemented!()
    }

    async fn release(
        &self,
        _req: Request,
        _inode: u64,
        _fh: u64,
        _flags: u32,
        _lock_owner: u64,
        _flush: bool,
    ) -> Result<()> {
        unimplemented!()
    }

    async fn fsync(&self, _req: Request, _inode: u64, _fh: u64, _datasync: bool) -> Result<()> {
        unimplemented!()
    }

    async fn flush(&self, _req: Request, _inode: u64, _fh: u64, _lock_owner: u64) -> Result<()> {
        unimplemented!()
    }

    async fn access(&self, _req: Request, _inode: u64, _mask: u32) -> Result<()> {
        unimplemented!()
    }

    async fn create(
        &self,
        _req: Request,
        parent: u64,
        name: &OsStr,
        mode: u32,
        flags: u32,
    ) -> Result<ReplyCreated> {
        unimplemented!()
    }

    async fn interrupt(&self, _req: Request, _unique: u64) -> Result<()> {
        unimplemented!()
    }

    async fn fallocate(
        &self,
        _req: Request,
        inode: u64,
        _fh: u64,
        offset: u64,
        length: u64,
        _mode: u32,
    ) -> Result<()> {
        unimplemented!()
    }

    async fn readdirplus(
        &self,
        _req: Request,
        parent: u64,
        _fh: u64,
        offset: u64,
        _lock_owner: u64,
    ) -> Result<ReplyDirectoryPlus<Self::DirEntryPlusStream>> {
        unimplemented!()
    }

    async fn rename2(
        &self,
        req: Request,
        parent: u64,
        name: &OsStr,
        new_parent: u64,
        new_name: &OsStr,
        _flags: u32,
    ) -> Result<()> {
        self.rename(req, parent, name, new_parent, new_name).await
    }

    async fn lseek(
        &self,
        _req: Request,
        inode: u64,
        _fh: u64,
        offset: u64,
        whence: u32,
    ) -> Result<ReplyLSeek> {
        unimplemented!()
    }

    async fn copy_file_range(
        &self,
        req: Request,
        inode: u64,
        fh_in: u64,
        off_in: u64,
        inode_out: u64,
        fh_out: u64,
        off_out: u64,
        length: u64,
        flags: u64,
    ) -> Result<ReplyCopyFileRange> {
        unimplemented!()
    }
}

pub fn log_init() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
}

