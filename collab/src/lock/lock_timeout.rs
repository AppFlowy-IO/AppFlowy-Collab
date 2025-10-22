use std::ops::Deref;
use std::time::Duration;

use crate::error::CollabError;

pub const DEFAULT_RWLOCK_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug)]
#[repr(transparent)]
pub struct RwLock<T: ?Sized> {
  inner: tokio::sync::RwLock<T>,
}

impl<T: ?Sized> RwLock<T> {
  pub fn new(inner: T) -> Self
  where
    T: Sized,
  {
    Self {
      inner: tokio::sync::RwLock::new(inner),
    }
  }

  pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, T> {
    match tokio::time::timeout(DEFAULT_RWLOCK_TIMEOUT, self.inner.read()).await {
      Ok(guard) => guard,
      Err(_) => panic!(
        "Trying to obtain read lock timed out after {:?}",
        DEFAULT_RWLOCK_TIMEOUT
      ),
    }
  }

  pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, T> {
    match tokio::time::timeout(DEFAULT_RWLOCK_TIMEOUT, self.inner.write()).await {
      Ok(guard) => guard,
      Err(_) => panic!(
        "Trying to obtain read lock timed out after {:?}",
        DEFAULT_RWLOCK_TIMEOUT
      ),
    }
  }

  pub async fn read_err(&self) -> Result<tokio::sync::RwLockReadGuard<'_, T>, CollabError> {
    match tokio::time::timeout(DEFAULT_RWLOCK_TIMEOUT, self.inner.read()).await {
      Ok(guard) => Ok(guard),
      Err(_) => Err(CollabError::RwLockReadTimeout(DEFAULT_RWLOCK_TIMEOUT)),
    }
  }

  pub async fn write_err(&self) -> Result<tokio::sync::RwLockWriteGuard<'_, T>, CollabError> {
    match tokio::time::timeout(DEFAULT_RWLOCK_TIMEOUT, self.inner.write()).await {
      Ok(guard) => Ok(guard),
      Err(_) => Err(CollabError::RwLockWriteTimeout(DEFAULT_RWLOCK_TIMEOUT)),
    }
  }
}

impl<T: ?Sized> Deref for RwLock<T> {
  type Target = tokio::sync::RwLock<T>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<T> From<T> for RwLock<T> {
  fn from(value: T) -> Self {
    Self::new(value)
  }
}

impl<T: Default> Default for RwLock<T> {
  fn default() -> Self {
    Self::new(T::default())
  }
}

#[derive(Debug)]
pub struct Mutex<T: ?Sized> {
  inner: tokio::sync::Mutex<T>,
}

impl<T: ?Sized> Mutex<T> {
  pub fn new(inner: T) -> Self
  where
    T: Sized,
  {
    Self {
      inner: tokio::sync::Mutex::new(inner),
    }
  }

  pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, T> {
    match tokio::time::timeout(DEFAULT_RWLOCK_TIMEOUT, self.inner.lock()).await {
      Ok(guard) => guard,
      Err(_) => panic!(
        "Trying to obtain lock timed out after {:?}",
        DEFAULT_RWLOCK_TIMEOUT
      ),
    }
  }

  pub async fn lock_err(&self) -> Result<tokio::sync::MutexGuard<'_, T>, CollabError> {
    match tokio::time::timeout(DEFAULT_RWLOCK_TIMEOUT, self.inner.lock()).await {
      Ok(guard) => Ok(guard),
      Err(_) => Err(CollabError::MutexLockTimeout(DEFAULT_RWLOCK_TIMEOUT)),
    }
  }
}

impl<T: ?Sized> Deref for Mutex<T> {
  type Target = tokio::sync::Mutex<T>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<T> From<T> for Mutex<T> {
  fn from(value: T) -> Self {
    Self::new(value)
  }
}

impl<T: Default> Default for Mutex<T> {
  fn default() -> Self {
    Self::new(T::default())
  }
}

#[cfg(test)]
mod test {
  use crate::lock::RwLock;
  use crate::preclude::Collab;
  use std::borrow::BorrowMut;
  use std::sync::Arc;

  #[test]
  fn trait_casting() {
    type CollabRef = Arc<RwLock<dyn BorrowMut<Collab> + Send + Sync + 'static>>;
    let collab = Arc::new(RwLock::new(Collab::new(
      0,
      uuid::Uuid::new_v4(),
      "device",
      1,
    )));
    let _collab_ref: CollabRef = collab as CollabRef;
  }
}
