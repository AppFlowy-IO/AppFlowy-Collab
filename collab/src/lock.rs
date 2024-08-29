use std::ops::Deref;
use std::time::Duration;

pub const DEFAULT_RWLOCK_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug)]
pub struct RwLock<T> {
  inner: tokio::sync::RwLock<T>,
  timeout: Duration,
}

impl<T> RwLock<T> {
  pub fn new(inner: T, timeout: Duration) -> Self {
    Self {
      inner: tokio::sync::RwLock::new(inner),
      timeout,
    }
  }

  pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, T> {
    match tokio::time::timeout(self.timeout, self.inner.read()).await {
      Ok(guard) => guard,
      Err(_) => panic!(
        "Trying to obtain read lock timed out after {:?}",
        self.timeout
      ),
    }
  }

  pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, T> {
    match tokio::time::timeout(self.timeout, self.inner.write()).await {
      Ok(guard) => guard,
      Err(_) => panic!(
        "Trying to obtain read lock timed out after {:?}",
        self.timeout
      ),
    }
  }

  pub async fn read_err(&self) -> Result<tokio::sync::RwLockReadGuard<'_, T>, RwLockError> {
    match tokio::time::timeout(self.timeout, self.inner.read()).await {
      Ok(guard) => Ok(guard),
      Err(_) => Err(RwLockError::ReadTimeout(self.timeout)),
    }
  }

  pub async fn write_err(&self) -> Result<tokio::sync::RwLockWriteGuard<'_, T>, RwLockError> {
    match tokio::time::timeout(self.timeout, self.inner.write()).await {
      Ok(guard) => Ok(guard),
      Err(_) => Err(RwLockError::WriteTimeout(self.timeout)),
    }
  }
}

impl<T> Deref for RwLock<T> {
  type Target = tokio::sync::RwLock<T>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<T> From<T> for RwLock<T> {
  fn from(value: T) -> Self {
    Self::new(value, DEFAULT_RWLOCK_TIMEOUT)
  }
}

impl<T: Default> Default for RwLock<T> {
  fn default() -> Self {
    Self::new(T::default(), DEFAULT_RWLOCK_TIMEOUT)
  }
}

#[derive(Debug, thiserror::Error)]
pub enum RwLockError {
  #[error("Read lock timeout: {0:?}")]
  ReadTimeout(Duration),
  #[error("Write lock timeout: {0:?}")]
  WriteTimeout(Duration),
}

#[derive(Debug)]
pub struct Mutex<T> {
  inner: tokio::sync::Mutex<T>,
  timeout: Duration,
}

impl<T> Mutex<T> {
  pub fn new(inner: T, timeout: Duration) -> Self {
    Self {
      inner: tokio::sync::Mutex::new(inner),
      timeout,
    }
  }

  pub async fn lock_unsafe(&self) -> tokio::sync::MutexGuard<'_, T> {
    match tokio::time::timeout(self.timeout, self.inner.lock()).await {
      Ok(guard) => guard,
      Err(_) => panic!("Trying to obtain lock timed out after {:?}", self.timeout),
    }
  }

  pub async fn lock_err(&self) -> Result<tokio::sync::MutexGuard<'_, T>, MutexError> {
    match tokio::time::timeout(self.timeout, self.inner.lock()).await {
      Ok(guard) => Ok(guard),
      Err(_) => Err(MutexError::LockTimeout(self.timeout)),
    }
  }
}

impl<T> Deref for Mutex<T> {
  type Target = tokio::sync::Mutex<T>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<T> From<T> for Mutex<T> {
  fn from(value: T) -> Self {
    Self::new(value, DEFAULT_RWLOCK_TIMEOUT)
  }
}

impl<T: Default> Default for Mutex<T> {
  fn default() -> Self {
    Self::new(T::default(), DEFAULT_RWLOCK_TIMEOUT)
  }
}

#[derive(Debug, thiserror::Error)]
pub enum MutexError {
  #[error("Lock timeout: {0:?}")]
  LockTimeout(Duration),
}
