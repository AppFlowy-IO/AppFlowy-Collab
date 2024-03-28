# Understanding the Architecture of AppFlowy

The presented PlantUML diagrams outline the architecture and workflow of a collaborative
document editor, referred to as the AppFlowy application. It highlights the roles of
different components and how they interact to create, open, edit, and synchronize documents.

### Architecture of AppFlowy

![](collab_object.png)

At its core, the AppFlowy application comprises three essential components: `flowy-folder`, `flowy-database`, and `flowy-document`. Alongside these, the `Collab` and `CollabPlugins` components play pivotal roles in data management and synchronization, connecting the core components of AppFlowy to a variety of data storage and synchronization plugins.

Designed with flexibility in mind, AppFlowy is engineered to interface seamlessly with various databases via `CollabPlugins`. At present, it supports RocksDB and Supabase. RocksDB was chosen for its high performance and availability, which makes it an excellent local storage solution.

The core components of the AppFlowy application interact with their corresponding elements within the `Collab` component. This `Collab` component then interfaces with the `CollabPlugins`. The modularity of AppFlowy's architecture allows for its functionality to be extended through the integration of new plugins into the `CollabPlugins` component.

For example, the integration of specific plugins could enable the storage of collaborative data in services like AWS or Firebase. Peer-to-peer synchronization could be made possible through the `RealtimePlugin`, while the `ContentIndexingPlugin` could be used to index the content of the collaboration, thereby supporting search functionality.

![](collab_object-CollabPlugins.png)

To illustrate how collaboration works within AppFlowy, let's walk through the process of creating a document. The other kind of collab object workflow is similar to this one, so we will not go through it in detail.

### Creating a Document

The creation of a new document in AppFlowy involves a series of steps, initiated by the user and facilitated by several components of the application.

1. The user initiates the process by clicking on the 'Create Document' button.
2. `flowy_folder` responds by creating a view with the specified document type.
3. Subsequently, `flowy_document` generates a document using the ID of the view.
4. These updates are then propagated to all plugins through `collab_document`.
5. `RocksdbDiskPlugin` captures these updates and saves them to the local disk.
6. Finally, `SupabaseDBPlugin` sends the updates to the server, ensuring that the document is stored and ready for collaboration.

![](collab_object-Create_Document.png)

### Opening a Document

Opening a document and keeping it in sync with the server is a multi-step process:

1. User opens the document.
2. `Collab` calls the `did_init` method of all plugins.
3. `SupabaseDBPlugin` sends an initial synchronization request to the server.
4. The server sends back an initial synchronization response, which is received by the `SupabaseDBPlugin`.

![](collab_object-Open_Document.png)
### Editing a Document

Editing a document is an interactive process that involves user action and several components of the system:

1. User types 'abc'.
2. `collab_document` creates an update containing 'abc'.
3. Updates get saved locally via `RocksdbDiskPlugin`.
4. Updates get pushed to send queue via `SupabaseDBPlugin`.
5. The updates are sent to the server in order.

![](collab_object-Edit_Document.png)

### Document Synchronization

The real-time synchronization of a document across different users involves the following steps:

1. User1 types 'abc'.
2. `Collab` creates an update containing 'abc'.
3. `SupabaseDBPlugin` sends the update to the server.
4. The server acknowledges the receipt of the update.
5. The server broadcasts the update.
6. Other users (User2, User3) receive the update.
7. Users apply the update.
8. UI is refreshed to reflect the updates.

![](collab_object-Sync_Document.png)

## New Collab Object
Each collab object is using the `collab` crate that is built on top of the [yrs](https://docs.rs/yrs/latest/yrs/) to build
its collaboration structure. Let's say we want to create a new collab object called `UserAwareness`. It stores the data associated with user. For example, the appearance settings and the reminder setting. The `UserAwareness` can
be synced between different devices for the given user_id.

The `UserAwareness` contains the following fields, as shown below:

![](create_collab_object-UserAwareness.png)

- `appearance_settings` is a [yrs map](https://docs.rs/yrs/latest/yrs/types/map/struct.MapRef.html) that contains the user's appearance settings.
- `reminders` is a [yrs array](https://docs.rs/yrs/latest/yrs/types/array/struct.ArrayRef.html) that contains the user's reminders. Each reminder can be serialized or deserialized using the [yrs map](https://docs.rs/yrs/latest/yrs/types/map/struct.MapRef.html).

Now, let's explore how to create the `UserAwareness` collab object.

```rust

pub struct UserAwareness {
    container: MapRefWrapper,
    appearance_settings: AppearanceSettings,
    reminders: Reminders,
}

impl UserAwareness {
    pub fn create(collab: Arc<MutexCollab>) -> Self {
        let collab_guard = collab.lock();
        let (container, appearance_settings, reminders) = collab_guard.with_transact_mut(|txn| {
            let awareness = collab_guard.insert_map_with_txn_if_not_exist(txn, USER);

            let appearance_settings_container =
                awareness.insert_map_with_txn_if_not_exist(txn, APPEARANCE_SETTINGS);
            let appearance_settings = AppearanceSettings {
                container: appearance_settings_container,
            };

            let reminder_container =
                awareness.insert_array_if_not_exist_with_txn::<Reminder>(txn, REMINDERS, vec![]);
            let reminders = Reminders {
                container: reminder_container,
            };

            (awareness, appearance_settings, reminders)
        });
        Self {
            container,
            appearance_settings,
            reminders,
        }
    }

    /// Adds a new reminder to the `UserAwareness` object.
    ///
    /// # Arguments
    ///
    /// * `reminder` - The `Reminder` object to be added.
    pub fn add_reminder(&self, reminder: Reminder) {
        self.reminders.add(reminder);
    }

    /// Removes an existing reminder from the `UserAwareness` object.
    ///
    /// # Arguments
    ///
    /// * `reminder_id` - A string reference to the ID of the reminder to be removed.
    pub fn remove_reminder(&self, reminder_id: &str) {
        self.reminders.remove(reminder_id);
    }

    /// Updates an existing reminder in the `UserAwareness` object.
    ///
    /// # Arguments
    ///
    /// * `reminder_id` - A string reference to the ID of the reminder to be updated.
    /// * `f` - A function or closure that takes `ReminderUpdate` as its argument and implements the changes to the reminder.
    pub fn update_reminder<F>(&self, reminder_id: &str, f: F)
        where
            F: FnOnce(ReminderUpdate),
    {
        self.reminders.update_reminder(reminder_id, f);
    }
}
```
Utilizing the `UserAwareness` object is simple and straightforward. A new `UserAwareness` object can be instantiated by calling the `create` method. Subsequently, reminders can be added, removed, or updated using the `add_reminder`, `remove_reminder`, and `update_reminder` methods, respectively. Each `UserAwareness` object is associated with a unique user ID, stored in `Arc<MutexCollab>`.

In situations where a user logs into multiple devices using the same user ID, the `UserAwareness` object is synchronized across all these devices. For instance, creating a new reminder triggers an update, which is then sent to the server. The server, in turn, broadcasts this update to all connected devices through the realtime service (WebSocket). Each device subsequently applies this update to its `UserAwareness` object, ensuring consistency across all devices.

Moreover, changes in the `Reminders` object can be subscribed to, enabling the user interface to refresh the reminder list whenever the `Reminders` object undergoes a change.

![](create_collab_object-CreateReminder.png)
