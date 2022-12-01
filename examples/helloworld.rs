// Copyright 2021 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

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
use fxfs::platform::linux::fuse_fs::{FuseFs, FuseStrParser};
use fxfs::platform::linux::fuse_handler::log_init;
use libc::mode_t;
use tokio::sync::RwLock;
use tracing::Level;
use fxfs::object_store::{Directory, HandleOptions, ObjectDescriptor, ObjectStore};
use fxfs::object_store::transaction::{Options, TransactionHandler};


fn default_request() -> Request {
    Request {
        unique: 0,
        uid: 0,
        gid: 0,
        pid: 0
    }
}


#[tokio::main(flavor = "current_thread")]
async fn main() {
    log_init();

    let args = env::args_os().skip(1).take(1).collect::<Vec<_>>();

    let mount_path = args.first();

    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };

    let mut mount_options = MountOptions::default();
    // .allow_other(true)
    mount_options
        .fs_name("fxfs")
        .force_readdir_plus(true)
        .uid(uid)
        .gid(gid);

    let fs = FuseFs::new_faked().await;
    let store_object_id = fs.default_store().await.unwrap().store_object_id();
    let dir = fs.root_dir().await;
    let dir_object_id = dir.unwrap().object_id();

    let mut transaction = fs.fs
        .clone()
        .new_transaction(&[], Options::default())
        .await
        .expect("new_transaction failed");
    let dir = Directory::create(&mut transaction, &fs.fs.root_store())
        .await
        .expect("create failed");

    let child_dir = dir
        .create_child_dir(&mut transaction, "foo")
        .await
        .expect("create_child_dir failed");

    transaction.commit().await.expect("commit failed");
    let _child_dir_file =
        ObjectStore::open_object(&fs.fs.root_store(), child_dir.object_id(), HandleOptions::default(), None)
            .await
            .expect("open object failed");

    // let res = fs.open_dir(dir_object_id).await;

    let res = fs.mkdir(default_request(), dir_object_id, OsStr::new("dog"), 0, 0).await;
    println!("{:?}", res);
    //
    // // let x= fs.lookup(default_request(), object_id, OsStr::new("dog")).await.unwrap();
    // let dir = fs.open_dir(object_id).await.unwrap();
    // let (_, object_descriptor) = dir.lookup(OsStr::new("dog").parse_str().unwrap()).await.unwrap().unwrap();
    // assert_eq!(object_descriptor, ObjectDescriptor::Directory);

    // let mount_path = mount_path.expect("no mount point specified");
    // Session::new(mount_options)
    //     .mount_with_unprivileged(fs, mount_path)
    //     .await
    //     .unwrap()
    //     .await
    //     .unwrap();
}
