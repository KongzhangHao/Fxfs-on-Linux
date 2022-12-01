use crate::{
    object_store::ObjectDescriptor,
    platform::linux::fuse_fs::{FuseFs, FuseStrParser},
};
use fuse3::raw::{Filesystem, Request};
use std::ffi::OsStr;

const DEFAULT_FILE_MODE: u32 = 0o755;

fn faked_request() -> Request {
    Request {
        unique: 0,
        uid: 0,
        gid: 0,
        pid: 0,
    }
}

pub async fn test_mkdir() {
    let fs = FuseFs::new_faked().await;
    let dir = fs.root_dir().await.expect("root_dir failed");

    fs.mkdir(
        faked_request(),
        dir.object_id(),
        OsStr::new("foo"),
        DEFAULT_FILE_MODE,
        0,
    )
    .await
    .expect("mkdir failed");
    let (child_id, child_descriptor) = dir
        .lookup(OsStr::new("foo").parse_str().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(child_descriptor, ObjectDescriptor::Directory);

    fs.mkdir(
        faked_request(),
        child_id,
        OsStr::new("bar"),
        DEFAULT_FILE_MODE,
        0,
    )
    .await
    .expect("mkdir failed");
    let child_dir = fs.open_dir(child_id).await.expect("open_dir failed");
    let (_, child_child_descriptor) = child_dir
        .lookup(OsStr::new("bar").parse_str().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(child_child_descriptor, ObjectDescriptor::Directory);
}

pub async fn test_rmdir() {
    let fs = FuseFs::new_faked().await;
    let dir = fs.root_dir().await.expect("root_dir failed");

    fs.mkdir(
        faked_request(),
        dir.object_id(),
        OsStr::new("foo"),
        DEFAULT_FILE_MODE,
        0,
    )
    .await
    .expect("mkdir failed");
    let (child_id, child_descriptor) = dir
        .lookup(OsStr::new("foo").parse_str().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(child_descriptor, ObjectDescriptor::Directory);

    fs.rmdir(faked_request(), child_id, OsStr::new("foo"))
        .await
        .expect("rmdir failed");
    let result = dir
        .lookup(OsStr::new("foo").parse_str().unwrap())
        .await
        .unwrap();
    assert_eq!(result, None);
}
