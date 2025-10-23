#![cfg(feature = "plugins")]

#[cfg(feature = "plugins")]
mod disk;

pub fn setup_log() {
  use tracing_subscriber::util::SubscriberInitExt;
  static START: std::sync::Once = std::sync::Once::new();
  START.call_once(|| {
    let level = "trace";
    let mut filters = vec![];
    filters.push(format!("collab_persistence={}", level));
    filters.push(format!("collab={}", level));
    filters.push(format!("collab_sync={}", level));
    filters.push(format!("collab::plugins={}", level));
    unsafe {
      std::env::set_var("RUST_LOG", filters.join(","));
    }

    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
      .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
      .with_ansi(true)
      .finish();
    subscriber.try_init().unwrap();
  });
}
