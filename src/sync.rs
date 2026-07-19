#[cfg(feature = "std")]
pub use parking_lot::{Mutex, RwLock};

#[cfg(not(feature = "std"))]
pub use spin::{Mutex, RwLock};
