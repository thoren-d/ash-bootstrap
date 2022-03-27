mod device;
mod error;
mod extensions;
mod instance;
mod swapchain;
pub(crate) mod util;

pub use device::Device;
pub use device::DeviceBuilder;
pub use device::PreferredDevice;
pub use error::Error;
pub use extensions::DeviceExtension;
pub use extensions::DeviceExtensionLoader;
pub use extensions::InstanceExtension;
pub use extensions::InstanceExtensionLoader;
pub use instance::{Instance, InstanceBuilder};
pub use swapchain::{Swapchain, SwapchainBuilder};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
