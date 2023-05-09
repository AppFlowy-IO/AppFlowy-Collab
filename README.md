# AppFlowy-Collab 

The`AppFlowy-Collab` is a project that support the collaborative features of AppFlowy. It is a collection of
crates:

* collab
* collab-database
* collab-document
* collab-folder
* collab-persistence
* collab-plugins
* collab-sync

![architecture.png](./resources/crate_arch.png)

These crates are currently under active development and is evolving rapidly. It is a work in progress and is being
iterated on at a fast pace to improve its features and functionality. As such, the project may still have some bugs
and limitations, and its API may change frequently as new features are added and existing ones are refined.


## collab
The `collab` crate is build on top of the [yrs](https://docs.rs/yrs/latest/yrs/) crate to provide a higher level
abstraction for the collaborative features of AppFlowy. It provides a simple API for creating and managing
collaborative documents.

## collab-database
The `collab-database` crate provides a simple API for creating and managing collaborative databases. It is built on
top of the `collab` crate.

## collab-document
The `collab-document` crate provides a simple API for creating and managing collaborative documents. It is built on
top of the `collab` crate.

## collab-folder
The `collab-folder` crate provides a simple API for creating and managing collaborative folders. It is built on top
of the `collab` crate.

## collab-plugins
The collab-plugins contains list of plugins that can be used with the `collab` crate. Currently, it has two plugins:
the `disk plugin` that use collab-persistence to persist the collaborative documents to disk, and the `sync plugin` that
use collab-sync to sync the collaborative documents to a remote server.

## collab-persistence
The `collab-persistence` uses [rocksdb](https://docs.rs/rocksdb/latest/rocksdb/) or [sled](https://github.com/spacejam/sled) 
to implement a persistence layer for the `collab` crate. It is easy to extend to support other key/value storage.

## collab-sync
The `collab-sync` supports syncing the collaborative documents to a remote server.
