use std::collections::BTreeMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, Cursor, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::vec::IntoIter;

use async_trait::async_trait;
use bytes::{Buf, BytesMut};
use fuse3::raw::prelude::*;
use fuse3::{Errno, MountOptions, Result};
use futures_util::stream;
use futures_util::stream::{Empty, Iter};
use futures_util::StreamExt;
use fxfs::filesystem::{Filesystem, FxFilesystem, SyncOptions};
use fxfs::object_store::transaction::{Options, TransactionHandler};
use fxfs::object_store::{Directory, HandleOptions, ObjectDescriptor, ObjectStore};
use fxfs::platform::linux::fuse_handler::{log_init, FuseFs};
use fxfs::platform::linux::mem_fs::Fs;
use libc::mode_t;
use storage_device::fake_device::FakeDevice;
use storage_device::DeviceHolder;
use tokio::sync::RwLock;
use tracing::Level;

const TEST_DEVICE_BLOCK_SIZE: u32 = 512;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let device = DeviceHolder::new(FakeDevice::new(8192, TEST_DEVICE_BLOCK_SIZE));
    let fs = FxFilesystem::new_empty(device)
        .await
        .expect("new_empty failed");
    let object_id = {
        let mut transaction = fs
            .clone()
            .new_transaction(&[], Options::default())
            .await
            .expect("new_transaction failed");
        let dir = Directory::create(&mut transaction, &fs.root_store())
            .await
            .expect("create failed");

        let child_dir = dir
            .create_child_dir(&mut transaction, "foo")
            .await
            .expect("create_child_dir failed");
        let _child_dir_file = child_dir
            .create_child_file(&mut transaction, "bar")
            .await
            .expect("create_child_file failed");
        let _child_file = dir
            .create_child_file(&mut transaction, "baz")
            .await
            .expect("create_child_file failed");
        dir.add_child_volume(&mut transaction, "corge", 100)
            .await
            .expect("add_child_volume failed");
        transaction.commit().await.expect("commit failed");
        fs.sync(SyncOptions::default()).await.expect("sync failed");
        dir.object_id()
    };

    fs.close().await.expect("Close failed");

    let device = fs.take_device().await;
    device.reopen(false);
    let fs = FxFilesystem::open(device).await.expect("open failed");
    {
        let dir = Directory::open(&fs.root_store(), object_id)
            .await
            .expect("open failed");
        let (object_id, object_descriptor) = dir
            .lookup("foo")
            .await
            .expect("lookup failed")
            .expect("not found");
        assert_eq!(object_descriptor, ObjectDescriptor::Directory);
        let child_dir = Directory::open(&fs.root_store(), object_id)
            .await
            .expect("open failed");
        let (object_id, object_descriptor) = child_dir
            .lookup("bar")
            .await
            .expect("lookup failed")
            .expect("not found");
        assert_eq!(object_descriptor, ObjectDescriptor::File);
        let _child_dir_file =
            ObjectStore::open_object(&fs.root_store(), object_id, HandleOptions::default(), None)
                .await
                .expect("open object failed");
        let (object_id, object_descriptor) = dir
            .lookup("baz")
            .await
            .expect("lookup failed")
            .expect("not found");
        assert_eq!(object_descriptor, ObjectDescriptor::File);
        let _child_file =
            ObjectStore::open_object(&fs.root_store(), object_id, HandleOptions::default(), None)
                .await
                .expect("open object failed");
        let (object_id, object_descriptor) = dir
            .lookup("corge")
            .await
            .expect("lookup failed")
            .expect("not found");
        assert_eq!(object_id, 100);
        if let ObjectDescriptor::Volume = object_descriptor {
        } else {
            panic!("wrong ObjectDescriptor");
        }

        assert_eq!(dir.lookup("qux").await.expect("lookup failed"), None);
    }
    fs.close().await.expect("Close failed");
}
