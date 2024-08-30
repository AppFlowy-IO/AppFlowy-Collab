#[cfg(not(feature = "lock_timeout"))]
pub type Mutex<T> = tokio::sync::Mutex<T>;
#[cfg(not(feature = "lock_timeout"))]
pub type RwLock<T> = tokio::sync::RwLock<T>;

#[cfg(feature = "lock_timeout")]
mod lock_timeout;

#[cfg(feature = "lock_timeout")]
pub use lock_timeout::Mutex;
#[cfg(feature = "lock_timeout")]
pub use lock_timeout::RwLock;
