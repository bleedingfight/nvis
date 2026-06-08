pub mod nsys;

#[cfg(feature = "ncu")]
pub mod ncu;

#[cfg(feature = "torch")]
pub mod torch;

use std::sync::Arc;
use crate::core::registry::Registry;

pub fn register_all(registry: &mut Registry) {
    registry.register_backend(Arc::new(nsys::NsysBackend));

    #[cfg(feature = "ncu")]
    registry.register_backend(Arc::new(ncu::NcuBackend));

    #[cfg(feature = "torch")]
    registry.register_backend(Arc::new(torch::TorchBackend));
}
