// Copyright 2021 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::{
    filesystem::{Filesystem, SyncOptions},
    object_handle::{ObjectHandle, ObjectHandleExt, ReadObjectHandle, WriteObjectHandle},
    object_store::{
        directory::{replace_child, ReplacedChild},
        transaction::{Options, TransactionHandler},
        Directory, ObjectDescriptor, Timestamp,
    },
    platform::linux::{
        attr::{create_dir_attr, create_file_attr},
        errors::FuseErrorParser,
        fuse_fs::{FuseFs, FuseStrParser},
    },
};
use async_trait::async_trait;
use bytes::BytesMut;
use fuse3::{
    raw::prelude::{Filesystem as FuseFilesystem, *},
    Result,
};
use futures_util::{
    stream,
    stream::{Empty, Iter},
};
use libc::mq_getattr;
use std::{
    ffi::{OsStr, OsString},
    io::Write,
    time::Duration,
    vec::IntoIter,
};
use tracing::Level;

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
        let object = dir.lookup(name.parse_str()?).await.parse_error()?;

        if let Some((object_id, object_descriptor)) = object {
            Ok(ReplyEntry {
                ttl: TTL,
                attr: self
                    .create_object_attr(object_id, object_descriptor)
                    .await?,
                generation: 0,
            })
        } else {
            Err(libc::ENOENT.into())
        }
    }

    async fn mkdir(
        &self,
        _req: Request,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
    ) -> Result<ReplyEntry> {
        let dir = self.open_dir(parent).await?;
        if dir.lookup(name.parse_str()?).await.parse_error()?.is_none() {
            let mut transaction = self
                .fs
                .clone()
                .new_transaction(&[], Options::default())
                .await
                .parse_error()?;

            let child_dir = dir
                .create_child_dir(&mut transaction, name.parse_str()?)
                .await
                .parse_error()?;

            transaction.commit().await.parse_error()?;

            Ok(ReplyEntry {
                ttl: TTL,
                attr: self
                    .create_object_attr(child_dir.object_id(), ObjectDescriptor::Directory)
                    .await?,
                generation: 0,
            })
        } else {
            Err(libc::EEXIST.into())
        }
    }

    async fn unlink(&self, _req: Request, parent: u64, name: &OsStr) -> Result<()> {
        let dir = self.open_dir(parent).await?;
        if let Some((_, object_descriptor)) = dir.lookup(name.parse_str()?).await.parse_error()? {
            if object_descriptor == ObjectDescriptor::File {
                let mut transaction = self
                    .fs
                    .clone()
                    .new_transaction(&[], Options::default())
                    .await
                    .parse_error()?;
                let replaced_child =
                    replace_child(&mut transaction, None, (&dir, name.parse_str()?))
                        .await
                        .parse_error()?;
                transaction.commit().await.parse_error()?;

                if let ReplacedChild::File(object_id) = replaced_child {
                    self.fs
                        .graveyard()
                        .tombstone(dir.store().store_object_id(), object_id)
                        .await
                        .parse_error()?;
                }
                Ok(())
            } else {
                Err(libc::EISDIR.into())
            }
        } else {
            Err(libc::ENOENT.into())
        }
    }

    /// Do nothing if the directory is not empty.
    async fn rmdir(&self, _req: Request, parent: u64, name: &OsStr) -> Result<()> {
        let dir = self.open_dir(parent).await?;
        if let Some((_, object_descriptor)) = dir.lookup(name.parse_str()?).await.parse_error()? {
            if object_descriptor == ObjectDescriptor::Directory {
                let mut transaction = self
                    .fs
                    .clone()
                    .new_transaction(&[], Options::default())
                    .await
                    .parse_error()?;
                replace_child(&mut transaction, None, (&dir, name.parse_str()?))
                    .await
                    .parse_error()?;
                transaction.commit().await.parse_error()?;
                Ok(())
            } else {
                Err(libc::ENOTDIR.into())
            }
        } else {
            Err(libc::ENOENT.into())
        }
    }

    async fn rename(
        &self,
        _req: Request,
        parent: u64,
        name: &OsStr,
        new_parent: u64,
        new_name: &OsStr,
    ) -> Result<()> {
        let old_dir = self.open_dir(parent).await?;
        let new_dir = self.open_dir(new_parent).await?;

        if old_dir
            .lookup(name.parse_str()?)
            .await
            .parse_error()?
            .is_some()
        {
            let mut transaction = self
                .fs
                .clone()
                .new_transaction(&[], Options::default())
                .await
                .parse_error()?;
            let replaced_child = replace_child(
                &mut transaction,
                Some((&old_dir, name.parse_str()?)),
                (&new_dir, new_name.parse_str()?),
            )
            .await
            .parse_error()?;
            transaction.commit().await.parse_error()?;

            if let ReplacedChild::File(object_id) = replaced_child {
                self.fs
                    .graveyard()
                    .tombstone(new_dir.store().store_object_id(), object_id)
                    .await
                    .parse_error()?;
            }
            Ok(())
        } else {
            Err(libc::ENOENT.into())
        }
    }

    async fn open(&self, _req: Request, inode: u64, _flags: u32) -> Result<ReplyOpen> {
        if let Some(object_type) = self.get_object_type(inode).await? {
            if object_type == ObjectDescriptor::File {
                Ok(ReplyOpen { fh: 0, flags: 0 })
            } else {
                Err(libc::EISDIR.into())
            }
        } else {
            Err(libc::ENOENT.into())
        }
    }

    async fn read(
        &self,
        _req: Request,
        inode: u64,
        _fh: u64,
        offset: u64,
        size: u32,
    ) -> Result<ReplyData> {
        if let Some(object_type) = self.get_object_type(inode).await? {
            if object_type == ObjectDescriptor::File {
                let handle = self.get_object_handle(inode).await?;
                let mut out: Vec<u8> = Vec::new();
                let align = offset % self.fs.block_size();

                let mut buf = handle.allocate_buffer(handle.block_size() as usize);
                let mut ofs = offset - align;
                let mut len = size as u64 + align;
                loop {
                    let bytes = handle.read(ofs, buf.as_mut()).await.parse_error()?;
                    if len - ofs > bytes as u64 {
                        ofs += bytes as u64;
                        out.write_all(&buf.as_ref().as_slice()[..bytes])?;
                        if bytes as u64 != handle.block_size() {
                            break;
                        }
                    } else {
                        out.write_all(&buf.as_ref().as_slice()[..(len - ofs) as usize])?;
                        break;
                    }
                }
                Ok(ReplyData { data: out.into() })
            } else {
                Err(libc::EISDIR.into())
            }
        } else {
            Err(libc::ENOENT.into())
        }
    }

    /// Does the offset automatically round down to the multiply of block size?
    async fn write(
        &self,
        _req: Request,
        inode: u64,
        _fh: u64,
        offset: u64,
        mut data: &[u8],
        _flags: u32,
    ) -> Result<ReplyWrite> {
        if let Some(object_type) = self.get_object_type(inode).await? {
            if object_type == ObjectDescriptor::File {
                let handle = self.get_object_handle(inode).await?;
                let mut buf = handle.allocate_buffer(data.len());
                buf.as_mut_slice().copy_from_slice(data);
                handle
                    .write_or_append(Some(offset), buf.as_ref())
                    .await
                    .parse_error()?;
                handle.flush().await.parse_error()?;
                Ok(ReplyWrite {
                    written: data.len() as u32,
                })
            } else {
                Err(libc::EISDIR.into())
            }
        } else {
            Err(libc::ENOENT.into())
        }
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
        if let Some(object_type) = self.get_object_type(inode).await? {
            Ok(ReplyAttr {
                ttl: TTL,
                attr: self.create_object_attr(inode, object_type).await?,
            })
        } else {
            Err(libc::ENOENT.into())
        }
    }

    async fn setattr(
        &self,
        _req: Request,
        inode: u64,
        _fh: Option<u64>,
        set_attr: SetAttr,
    ) -> Result<ReplyAttr> {
        if let Some(object_type) = self.get_object_type(inode).await? {
            let handle = self.get_object_handle(inode).await?;
            let mut transaction = self
                .fs
                .clone()
                .new_transaction(&[], Options::default())
                .await
                .parse_error()?;

            let ctime: Option<Timestamp> = match set_attr.ctime {
                Some(t) => Some(t.into()),
                None => None,
            };
            let mtime: Option<Timestamp> = match set_attr.mtime {
                Some(t) => Some(t.into()),
                None => None,
            };

            handle.write_timestamps(&mut transaction, ctime, mtime);
            transaction.commit().await.parse_error()?;

            Ok(ReplyAttr {
                ttl: TTL,
                attr: self.create_object_attr(inode, object_type).await?,
            })
        } else {
            Err(libc::ENOENT.into())
        }
    }

    async fn fsync(&self, _req: Request, _inode: u64, _fh: u64, _datasync: bool) -> Result<()> {
        Ok(self.fs.sync(SyncOptions::default()).await.parse_error()?)
    }

    async fn flush(&self, _req: Request, _inode: u64, _fh: u64, _lock_owner: u64) -> Result<()> {
        Err(libc::ENOTSUP.into())
    }

    async fn access(&self, _req: Request, _inode: u64, _mask: u32) -> Result<()> {
        Err(libc::ENOTSUP.into())
    }

    async fn create(
        &self,
        _req: Request,
        parent: u64,
        name: &OsStr,
        mode: u32,
        flags: u32,
    ) -> Result<ReplyCreated> {
        let dir = self.open_dir(parent).await?;
        if dir.lookup(name.parse_str()?).await.parse_error()?.is_none() {
            let mut transaction = self
                .fs
                .clone()
                .new_transaction(&[], Options::default())
                .await
                .parse_error()?;

            let child_file = dir
                .create_child_file(&mut transaction, name.parse_str()?)
                .await
                .parse_error()?;

            transaction.commit().await.parse_error()?;

            Ok(ReplyCreated {
                ttl: TTL,
                attr: self
                    .create_object_attr(child_file.object_id(), ObjectDescriptor::File)
                    .await?,
                generation: 0,
                fh: 0,
                flags,
            })
        } else {
            Err(libc::EEXIST.into())
        }
    }

    async fn interrupt(&self, _req: Request, _unique: u64) -> Result<()> {
        Err(libc::ENOTSUP.into())
    }

    /// Currently no support for offset
    async fn fallocate(
        &self,
        _req: Request,
        inode: u64,
        _fh: u64,
        offset: u64,
        length: u64,
        _mode: u32,
    ) -> Result<()> {
        if let Some(object_type) = self.get_object_type(inode).await? {
            if object_type == ObjectDescriptor::File {
                let handle = self.get_object_handle(inode).await?;
                handle.truncate(length).await.parse_error()?;
                handle.flush().await.parse_error()?;
                Ok(())
            } else {
                Err(libc::EISDIR.into())
            }
        } else {
            Err(libc::ENOENT.into())
        }
    }

    async fn readdirplus(
        &self,
        _req: Request,
        parent: u64,
        _fh: u64,
        offset: u64,
        _lock_owner: u64,
    ) -> Result<ReplyDirectoryPlus<Self::DirEntryPlusStream>> {
        if let Some(object_type) = self.get_object_type(parent).await? {
            if object_type == ObjectDescriptor::Directory {
                let parent_attr = self
                    .create_object_attr(parent, ObjectDescriptor::Directory)
                    .await?;
                let dir = self.open_dir(parent).await?;
                // TODO Currently using the attribute of the same dir. Need to change to the parent one.
                let grandparent = parent;
                let grandparent_attr = self
                    .create_object_attr(grandparent, ObjectDescriptor::Directory)
                    .await?;

                let mut children = vec![
                    Ok(DirectoryEntryPlus {
                        inode: parent,
                        generation: 0,
                        kind: FileType::Directory,
                        name: OsString::from("."),
                        offset: 1,
                        attr: parent_attr,
                        entry_ttl: TTL,
                        attr_ttl: TTL,
                    }),
                    Ok(DirectoryEntryPlus {
                        inode: grandparent,
                        generation: 0,
                        kind: FileType::Directory,
                        name: OsString::from(".."),
                        offset: 2,
                        attr: grandparent_attr,
                        entry_ttl: TTL,
                        attr_ttl: TTL,
                    }),
                ];

                let layer_set = dir.store().tree().layer_set();
                let mut merger = layer_set.merger();
                let mut iter = dir.iter(&mut merger).await.parse_error()?;
                let mut skip_ofs = offset;
                let mut entry_ofs = 3i64;

                while let Some((name, object_id, descriptor)) = iter.get() {
                    while skip_ofs > 0 {
                        skip_ofs -= 1;
                        continue;
                    }
                    let file_type = match descriptor {
                        ObjectDescriptor::File => FileType::RegularFile,
                        _ => FileType::Directory,
                    };

                    children.push(Ok(DirectoryEntryPlus {
                        inode: grandparent,
                        generation: 0,
                        kind: file_type,
                        name: OsString::from(name),
                        offset: entry_ofs,
                        attr: self
                            .create_object_attr(grandparent, descriptor.clone())
                            .await?,
                        entry_ttl: TTL,
                        attr_ttl: TTL,
                    }));
                    entry_ofs += 1;
                }

                Ok(ReplyDirectoryPlus {
                    entries: stream::iter(children),
                })
            } else {
                Err(libc::ENOTDIR.into())
            }
        } else {
            Err(libc::ENOENT.into())
        }
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
        Err(libc::ENOTSUP.into())
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
        Err(libc::ENOTSUP.into())
    }
}

pub fn log_init() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
}
