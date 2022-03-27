use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Vulkan Loading Error")]
    LoadingError(#[from] ash::LoadingError),
    #[error("Vulkan Error")]
    VulkanError(#[from] ash::vk::Result),
    #[error("No Suitable Devices Found")]
    NoSuitableDevices,
}
