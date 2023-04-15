use crate::blocks::{parse_event, BlockEvent};
use collab::preclude::{DeepEventsSubscription, DeepObservable, MapRefWrapper, Origin};

pub struct RootDeepSubscription {
  pub(crate) subscription: Option<DeepEventsSubscription>,
}

impl Default for RootDeepSubscription {
  fn default() -> Self {
    Self::new()
  }
}

impl RootDeepSubscription {
  pub fn new() -> Self {
    Self { subscription: None }
  }
  pub fn subscribe<F>(&mut self, root: &mut MapRefWrapper, callback: F)
  where
    F: Fn(&Vec<BlockEvent>, Option<&Origin>) + 'static,
  {
    let subscription = Some(root.observe_deep(move |txn, events| {
      let block_events = events
        .iter()
        .map(|deep_event| parse_event(txn, deep_event))
        .collect::<Vec<BlockEvent>>();

      callback(&block_events, txn.origin());
    }));
    self.subscription = subscription;
  }
}
