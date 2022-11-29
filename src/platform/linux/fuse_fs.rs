use crate::object_store::{ObjectKey, ObjectKind, ObjectValue};
use crate::{
    crypt::Crypt,
    filesystem::{FxFilesystem, OpenFxFilesystem},
    object_handle::{GetProperties, ObjectProperties},
    object_store::{
        transaction::Transaction, volume::root_volume, Directory, HandleOptions, ObjectDescriptor,
        ObjectStore, StoreObjectHandle,
    },
    platform::linux::{
        attr::{create_dir_attr, create_file_attr},
        errors::{cast_to_fuse_error, FuseErrorParser},
    },
};
use fuse3::{raw::prelude::*, Result};
use std::error::Error;
use std::ffi::OsStr;
use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
    time::Duration,
};
use storage_device::{fake_device::FakeDevice, DeviceHolder};
use tokio::sync::RwLock;

const TEST_DEVICE_BLOCK_SIZE: u32 = 512;
const DEFAULT_VOLUME_NAME: &str = "fuse";
const DEFAULT_FILE_MODE: u32 = 0o755;

pub struct FuseFs {
    pub fs: OpenFxFilesystem,
    crypt: Option<Arc<dyn Crypt>>,
}

impl FuseFs {
    pub async fn new_faked() -> Self {
        let device = DeviceHolder::new(FakeDevice::new(8192, TEST_DEVICE_BLOCK_SIZE));
        let crypt = None;
        FuseFs::new(device, crypt).await
    }

    pub async fn new(device: DeviceHolder, crypt: Option<Arc<dyn Crypt>>) -> Self {
        let fs = FxFilesystem::new_empty(device)
            .await
            .expect("new_empty failed");
        let root_volume = root_volume(fs.clone()).await.expect("root_volume failed");
        root_volume
            .new_volume(DEFAULT_VOLUME_NAME, crypt.clone())
            .await
            .expect("new_volume failed");

        Self { fs, crypt }
    }

    pub async fn default_store(&self) -> Result<Arc<ObjectStore>> {
        let root_volume = root_volume(self.fs.clone()).await.parse_error()?;
        root_volume
            .volume(DEFAULT_VOLUME_NAME, self.crypt.clone())
            .await
            .parse_error()
    }

    pub async fn root_dir(&self) -> Result<Directory<ObjectStore>> {
        let store = self.default_store().await?;
        Directory::open(&store, store.root_directory_object_id())
            .await
            .parse_error()
    }

    pub async fn open_dir(&self, object_id: u64) -> Result<Directory<ObjectStore>> {
        let store = self.default_store().await?;
        Directory::open(&store, object_id).await.parse_error()
    }

    pub async fn get_object_handle(
        &self,
        object_id: u64,
    ) -> Result<StoreObjectHandle<ObjectStore>> {
        let store = self.default_store().await?;
        ObjectStore::open_object(&store, object_id, HandleOptions::default(), None)
            .await
            .parse_error()
    }

    pub async fn get_object_properties(&self, object_id: u64) -> Result<ObjectProperties> {
        let handle = self.get_object_handle(object_id).await?;
        handle.get_properties().await.parse_error()
    }

    pub async fn get_object_type(&self, object_id: u64) -> Result<Option<ObjectDescriptor>> {
        let object_result = self
            .default_store()
            .await?
            .tree()
            .find(&ObjectKey::object(object_id))
            .await
            .parse_error()?;
        if let Some(object) = object_result {
            match object.value {
                ObjectValue::Object {
                    kind: ObjectKind::Directory { .. },
                    ..
                } => Ok(Some(ObjectDescriptor::Directory)),
                ObjectValue::Object {
                    kind: ObjectKind::File { .. },
                    ..
                } => Ok(Some(ObjectDescriptor::File)),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    pub async fn create_object_attr(
        &self,
        object_id: u64,
        object_type: ObjectDescriptor,
    ) -> Result<FileAttr> {
        let properties = self.get_object_properties(object_id).await?;
        match object_type {
            ObjectDescriptor::Directory => Ok(create_dir_attr(
                object_id,
                properties.allocated_size,
                properties.creation_time,
                properties.modification_time,
                DEFAULT_FILE_MODE,
            )),
            ObjectDescriptor::File => Ok(create_file_attr(
                object_id,
                properties.allocated_size,
                properties.creation_time,
                properties.modification_time,
                DEFAULT_FILE_MODE,
            )),
            ObjectDescriptor::Volume => Err(libc::ENOSYS.into()),
        }
    }
}

impl Debug for FuseFs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Debugging FuseFs").finish()
    }
}

pub trait FuseStrParser {
    fn parse_str(&self) -> Result<&str>;
}

impl FuseStrParser for OsStr {
    fn parse_str(&self) -> Result<&str> {
        if let Some(s) = self.to_str() {
            Ok(s)
        } else {
            Err(libc::EINVAL.into())
        }
    }
}
