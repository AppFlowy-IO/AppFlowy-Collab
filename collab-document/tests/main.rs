#[cfg(not(target_arch = "wasm32"))]
mod block_parser;
#[cfg(not(target_arch = "wasm32"))]
mod blocks;
#[cfg(not(target_arch = "wasm32"))]
mod document;
#[cfg(not(target_arch = "wasm32"))]
mod util;

#[cfg(not(target_arch = "wasm32"))]
mod conversions;

#[cfg(not(target_arch = "wasm32"))]
mod importer;
