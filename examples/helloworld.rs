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
use fxfs::platform::linux::fuse_handler::{log_init, FuseFs};
use fxfs::platform::linux::mem_fs::Fs;
use libc::mode_t;
use tokio::sync::RwLock;
use tracing::Level;

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
        .fs_name("memfs")
        .force_readdir_plus(true)
        .uid(uid)
        .gid(gid);

    let fs = FuseFs::new_faked().await;
    fs.fs.close().await.expect("Close failed");
    println!("!!!!!!");
    let fs = Fs::default();

    let mount_path = mount_path.expect("no mount point specified");
    Session::new(mount_options)
        .mount_with_unprivileged(fs, mount_path)
        .await
        .unwrap()
        .await
        .unwrap();
}
