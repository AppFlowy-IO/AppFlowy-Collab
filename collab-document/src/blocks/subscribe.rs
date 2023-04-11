use crate::blocks::{get_delta_from_event, BlockEvent};
use collab::preclude::{
  DeepEventsSubscription, DeepObservable, MapRefWrapper, Origin, PathSegment,
};

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
        .map(|deep_event| {
          let delta = get_delta_from_event(txn, deep_event);
          let path = deep_event
            .path()
            .iter()
            .map(|v| match v {
              PathSegment::Key(v) => v.to_string(),
              PathSegment::Index(v) => v.to_string(),
            })
            .collect();
          BlockEvent { path, delta }
        })
        .collect();

      callback(&block_events, txn.origin());
    }));
    self.subscription = subscription;
  }
}
