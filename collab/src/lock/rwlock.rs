use std::ops::Deref;
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

  pub async fn write_with_reason(&self, reason: &str) -> tokio::sync::RwLockWriteGuard<'_, T> {
    debug!("Acquiring write lock for reason: {}", reason);
    self.inner.write().await
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
