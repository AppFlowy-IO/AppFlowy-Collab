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
      let mut block_events = vec![];
      events.iter().for_each(|deep_event| {
        let delta = get_delta_from_event(txn, deep_event);

        let mut path = vec![];
        deep_event.path().iter().for_each(|v| match v {
          PathSegment::Key(v) => path.push(v.to_string()),
          PathSegment::Index(v) => path.push(v.to_string()),
        });

        let block_event = BlockEvent { path, delta };
        block_events.push(block_event);
      });

      callback(&block_events, txn.origin());
    }));
    self.subscription = subscription;
  }
}
