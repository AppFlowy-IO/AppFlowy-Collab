use std::sync::Once;

use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

mod mock_sync;
mod sync;
mod util;
mod disk;

pub fn setup_log() {
  static START: Once = Once::new();
  START.call_once(|| {
    std::env::set_var(
      "RUST_LOG",
      "collab_persistence=info,collab=trace,collab_sync=trace,collab_plugins=trace",
    );
    let subscriber = Subscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .with_ansi(true)
      .finish();
    subscriber.try_init().unwrap();
  });
}
