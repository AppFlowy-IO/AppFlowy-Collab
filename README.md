
# AppFlowy-Collab

`AppFlowy-Collab` is a project that aims to support the collaborative features of AppFlowy. The workspace now centers on
the `collab` crate, which houses database, document, folder, importer, plugin, and user functionality under a single
module tree.

![architecture.png](resources/crate_arch.png)

As the project is still a work in progress, it is rapidly evolving to improve its features and functionality. Therefore,
it may still have some bugs and limitations, and its API may change frequently as new features are added and existing
ones are refined.

## collab
The `collab` crate is built on top of the [yrs](https://docs.rs/yrs/latest/yrs/) crate, providing a higher level of
abstraction for the collaborative features of AppFlowy. It exposes cohesive modules:

- Entity definitions and protobuf types under `collab::entity`.
- Database, document, folder, importer, plugin, and user services under `collab::database`, `collab::document`,
  `collab::folder`, `collab::importer`, `collab::plugins`, and `collab::user`.

With everything consolidated, consumers only need to depend on the `collab` crate to access the full collaborative
feature set.
