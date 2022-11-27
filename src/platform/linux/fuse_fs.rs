use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use std::time::Duration;

use crate::crypt::Crypt;
use crate::filesystem::{FxFilesystem, OpenFxFilesystem};
use crate::object_handle::{GetProperties, ObjectProperties};
use crate::object_store::volume::root_volume;
use crate::object_store::{
    Directory, HandleOptions, ObjectDescriptor, ObjectStore, StoreObjectHandle,
};
use crate::platform::linux::attr::{create_dir_attr, create_file_attr};
use crate::platform::linux::errors::cast_to_fuse_error;
use fuse3::raw::prelude::*;
use fuse3::Result;
use storage_device::fake_device::FakeDevice;
use storage_device::DeviceHolder;
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

    pub async fn default_store(&self) -> Arc<ObjectStore> {
        let root_volume = root_volume(self.fs.clone())
            .await
            .expect("root_volume failed");
        let store = root_volume
            .volume(DEFAULT_VOLUME_NAME, self.crypt.clone())
            .await
            .expect("FUSE volume failed");
        store
    }

    pub async fn root_dir(&self) -> Directory<ObjectStore> {
        let store = self.default_store().await;
        let root_directory = Directory::open(&store, store.root_directory_object_id())
            .await
            .expect("Directory open failed");
        root_directory
    }

    pub async fn open_dir(&self, object_id: u64) -> Result<Directory<ObjectStore>> {
        let store = self.default_store().await;
        let dir_result = Directory::open(&store, object_id).await;

        if let Ok(dir) = dir_result {
            Ok(dir)
        } else {
            Err(cast_to_fuse_error(&dir_result.err().unwrap()))
        }
    }

    pub async fn get_object_handle(
        &self,
        object_id: u64,
    ) -> Result<StoreObjectHandle<ObjectStore>> {
        let store = self.default_store().await;
        let handle_result =
            ObjectStore::open_object(&store, object_id, HandleOptions::default(), None).await;

        if let Ok(handle) = handle_result {
            Ok(handle)
        } else {
            Err(cast_to_fuse_error(&handle_result.err().unwrap()))
        }
    }

    pub async fn get_object_properties(&self, object_id: u64) -> Result<ObjectProperties> {
        let handle = self.get_object_handle(object_id).await?;
        let properties_result = handle.get_properties().await;
        if let Ok(properties) = properties_result {
            Ok(properties)
        } else {
            Err(cast_to_fuse_error(&properties_result.err().unwrap()))
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
