use std::ops::{Deref, DerefMut};
use std::time::Duration;
use tokio::sync::TryLockError;
use tracing::debug;

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
    self.inner.read().await
  }

  pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, T> {
    self.inner.write().await
  }

  pub async fn write_with_reason(&self, reason: &str) -> RwLockWriteGuardWrapper<'_, T> {
    debug!("Acquiring write lock for reason: {}", reason);
    let guard = self.inner.write().await;
    RwLockWriteGuardWrapper {
      inner: guard,
      reason: reason.to_string(),
    }
  }

  pub async fn try_read_for_duration(
    &self,
    duration: Duration,
  ) -> Result<tokio::sync::RwLockReadGuard<'_, T>, TryLockError> {
    let start = tokio::time::Instant::now();

    loop {
      match self.inner.try_read() {
        Ok(guard) => return Ok(guard),
        Err(err) => {
          if start.elapsed() >= duration {
            return Err(err);
          }
          tokio::time::sleep(Duration::from_millis(10)).await;
        },
      }
    }
  }

  pub async fn try_write_for_duration(
    &self,
    duration: Duration,
  ) -> Result<tokio::sync::RwLockWriteGuard<'_, T>, TryLockError> {
    let start = tokio::time::Instant::now();

    loop {
      match self.inner.try_write() {
        Ok(guard) => return Ok(guard),
        Err(err) => {
          if start.elapsed() >= duration {
            return Err(err);
          }
          tokio::time::sleep(Duration::from_millis(10)).await;
        },
      }
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

pub struct RwLockWriteGuardWrapper<'a, T: ?Sized> {
  inner: tokio::sync::RwLockWriteGuard<'a, T>,
  reason: String,
}
impl<'a, T: ?Sized> Deref for RwLockWriteGuardWrapper<'a, T> {
  type Target = tokio::sync::RwLockWriteGuard<'a, T>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuardWrapper<'_, T> {
  #[inline]
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}

impl<T: ?Sized> Drop for RwLockWriteGuardWrapper<'_, T> {
  fn drop(&mut self) {
    debug!("Releasing write lock for reason: {}", self.reason);
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
