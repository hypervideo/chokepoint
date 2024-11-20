#![allow(unused_imports)]

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::*;
#[cfg(not(target_arch = "wasm32"))]
pub use tokio::time as tokio_time;
#[cfg(not(target_arch = "wasm32"))]
pub use tokio_util::time as tokio_util;
#[cfg(target_arch = "wasm32")]
pub use wasmtimer::{
    std::*,
    tokio as tokio_time,
    tokio_util,
};
