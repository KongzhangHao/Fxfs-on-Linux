// Copyright 2021 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::errors::FxfsError;
use fuse3::{Errno, Result};

pub fn cast_to_fuse_error(err: &anyhow::Error) -> Errno {
    if let Some(root_cause) = err.root_cause().downcast_ref::<FxfsError>() {
        match root_cause {
            FxfsError::Inconsistent => libc::EPIPE.into(),
            FxfsError::NotFound => libc::ENOENT.into(),
            FxfsError::NotDir => libc::ENOTDIR.into(),
            FxfsError::Deleted => libc::ENOENT.into(),
            FxfsError::AlreadyExists => libc::EEXIST.into(),
            FxfsError::Internal => libc::EPIPE.into(),
            FxfsError::NotFile => libc::EISDIR.into(),
            FxfsError::NotEmpty => libc::ENOTEMPTY.into(),
            FxfsError::ReadOnlyFilesystem => libc::EROFS.into(),
            FxfsError::NoSpace => libc::ENOSPC.into(),
            FxfsError::InvalidArgs => libc::EINVAL.into(),
            FxfsError::TooBig => libc::EFBIG.into(),
            FxfsError::InvalidVersion => libc::ENOTSUP.into(),
            FxfsError::JournalFlushError => libc::ENOTSUP.into(),
            FxfsError::NotSupported => libc::ENOTSUP.into(),
            FxfsError::AccessDenied => libc::ELIBACC.into(),
            FxfsError::OutOfRange => libc::ERANGE.into(),
            FxfsError::AlreadyBound => libc::EALREADY.into(),
            _ => libc::ENOTSUP.into(),
        }
    } else {
        libc::ENOTSUP.into()
    }
}

pub trait FuseErrorParser<S> {
    fn parse_error(self) -> Result<S>;
}

impl<S> FuseErrorParser<S> for std::result::Result<S, anyhow::Error> {
    fn parse_error(self) -> Result<S> {
        if let Ok(ret) = self {
            Ok(ret)
        } else {
            Err(cast_to_fuse_error(&self.err().unwrap()))
        }
    }
}
