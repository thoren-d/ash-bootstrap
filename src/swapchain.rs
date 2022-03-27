use std::sync::Arc;

use ash::vk;

use crate::{Device, Error};

#[derive(Clone)]
pub struct SwapchainBuilder {
    preferred_formats: Vec<vk::SurfaceFormatKHR>,
    preferred_modes: Vec<vk::PresentModeKHR>,
    extent: Option<(u32, u32)>,
    previous_swapchain: vk::SwapchainKHR,
    triple_buffered: bool,
    usage: vk::ImageUsageFlags,
}

pub struct Swapchain {
    device: Arc<Device>,
    swapchain: vk::SwapchainKHR,
    extent: vk::Extent2D,
    format: vk::SurfaceFormatKHR,
    image_views: Vec<vk::ImageView>,
    builder: SwapchainBuilder,
}

// These formats should have ~100% coverage, and don't
// require any extra effort to encode correctly.
const DEFAULT_PREFERRED_FORMATS: &[vk::SurfaceFormatKHR] = &[
    vk::SurfaceFormatKHR {
        format: vk::Format::B8G8R8A8_SRGB,
        color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
    },
    vk::SurfaceFormatKHR {
        format: vk::Format::R8G8B8A8_SRGB,
        color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
    },
    vk::SurfaceFormatKHR {
        format: vk::Format::A8B8G8R8_SRGB_PACK32,
        color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
    },
];

const DEFAULT_PREFERRED_MODES: &[vk::PresentModeKHR] = &[
    vk::PresentModeKHR::FIFO_RELAXED,
    vk::PresentModeKHR::FIFO,
    vk::PresentModeKHR::MAILBOX,
    vk::PresentModeKHR::IMMEDIATE,
];

impl SwapchainBuilder {
    pub fn new() -> SwapchainBuilder {
        SwapchainBuilder {
            preferred_formats: Vec::new(),
            preferred_modes: Vec::new(),
            extent: None,
            previous_swapchain: vk::SwapchainKHR::null(),
            triple_buffered: false,
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST,
        }
    }

    pub fn prefer_format(&mut self, format: vk::SurfaceFormatKHR) -> &mut Self {
        self.preferred_formats.push(format);
        self
    }

    pub fn prefer_mode(&mut self, mode: vk::PresentModeKHR) -> &mut Self {
        self.preferred_modes.push(mode);
        self
    }

    pub fn extent(&mut self, width: u32, height: u32) -> &mut Self {
        self.extent = Some((width, height));
        self
    }

    pub fn previous_swapchain(&mut self, previous: vk::SwapchainKHR) -> &mut Self {
        self.previous_swapchain = previous;
        self
    }

    pub fn triple_buffered(&mut self) -> &mut Self {
        self.triple_buffered = true;
        self
    }

    pub fn usage(&mut self, usage: vk::ImageUsageFlags) -> &mut Self {
        self.usage = usage;
        self
    }

    pub fn build(&self, device: Arc<Device>, surface: vk::SurfaceKHR) -> Result<Swapchain, Error> {
        unsafe {
            let instance = device.instance();
            let surface_ext = instance
                .extension::<ash::extensions::khr::Surface>()
                .unwrap();
            let swapchain_ext = device
                .extension::<ash::extensions::khr::Swapchain>()
                .unwrap();
            let capabilities = surface_ext
                .get_physical_device_surface_capabilities(device.physical_device(), surface)?;
            let formats = surface_ext
                .get_physical_device_surface_formats(device.physical_device(), surface)?;
            let modes = surface_ext
                .get_physical_device_surface_present_modes(device.physical_device(), surface)?;

            let format = self.pick_format(&formats);
            let mode = self.pick_mode(&modes);
            let extent = self.pick_extent(&capabilities);
            let image_count = self.pick_image_count(&capabilities);

            let create_info = vk::SwapchainCreateInfoKHR::builder()
                .clipped(true)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .image_array_layers(1)
                .image_color_space(format.color_space)
                .image_extent(extent)
                .image_format(format.format)
                .image_usage(self.usage)
                .min_image_count(image_count)
                .old_swapchain(self.previous_swapchain)
                .pre_transform(capabilities.current_transform)
                .present_mode(mode)
                .surface(surface);

            let queue_families = [
                device.graphics_queue().unwrap().0,
                device.present_queue().unwrap().0,
            ];

            let create_info = if queue_families[0] == queue_families[1] {
                create_info.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            } else {
                create_info
                    .image_sharing_mode(vk::SharingMode::CONCURRENT)
                    .queue_family_indices(&queue_families)
            };

            let swapchain = swapchain_ext.create_swapchain(&create_info, None)?;
            let images = swapchain_ext.get_swapchain_images(swapchain)?;
            let image_views = Self::create_image_views(&device, &images, format.format)?;

            Ok(Swapchain {
                device,
                swapchain,
                extent,
                format,
                image_views,
                builder: self.clone(),
            })
        }
    }

    fn pick_format(&self, formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
        for preferred in self
            .preferred_formats
            .iter()
            .chain(DEFAULT_PREFERRED_FORMATS.iter())
        {
            for supported in formats {
                if *preferred == *supported {
                    return *preferred;
                }
            }
        }

        formats[0]
    }

    fn pick_mode(&self, modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
        for preferred in self
            .preferred_modes
            .iter()
            .chain(DEFAULT_PREFERRED_MODES.iter())
        {
            for supported in modes {
                if *preferred == *supported {
                    return *preferred;
                }
            }
        }

        modes[0]
    }

    fn pick_extent(&self, capabilities: &vk::SurfaceCapabilitiesKHR) -> vk::Extent2D {
        if capabilities.current_extent.width != u32::MAX
            && capabilities.current_extent.height != u32::MAX
        {
            capabilities.current_extent
        } else {
            let (width, height) = self.extent.unwrap_or((800, 600));
            vk::Extent2D {
                width: width.clamp(
                    capabilities.min_image_extent.width,
                    capabilities.max_image_extent.width,
                ),
                height: height.clamp(
                    capabilities.min_image_extent.height,
                    capabilities.max_image_extent.height,
                ),
            }
        }
    }

    fn pick_image_count(&self, capabilities: &vk::SurfaceCapabilitiesKHR) -> u32 {
        let preference = if self.triple_buffered { 3 } else { 2 };
        preference.clamp(capabilities.min_image_count, capabilities.max_image_count)
    }

    unsafe fn create_image_views(
        device: &Device,
        images: &[vk::Image],
        format: vk::Format,
    ) -> Result<Vec<vk::ImageView>, Error> {
        let mut res = Vec::with_capacity(images.len());
        for image in images {
            let create_info = vk::ImageViewCreateInfo::builder()
                .components(Default::default())
                .format(format)
                .image(*image)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .view_type(vk::ImageViewType::TYPE_2D);
            let image_view = device.device().create_image_view(&create_info, None)?;
            res.push(image_view);
        }

        Ok(res)
    }
}

impl Default for SwapchainBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Swapchain {
    pub fn swapchain(&self) -> vk::SwapchainKHR {
        self.swapchain
    }

    pub fn extent(&self) -> vk::Extent2D {
        self.extent
    }

    pub fn format(&self) -> vk::SurfaceFormatKHR {
        self.format
    }

    pub fn image_views(&self) -> &[vk::ImageView] {
        &self.image_views
    }

    pub fn builder_mut(&mut self) -> &mut SwapchainBuilder {
        &mut self.builder
    }

    pub fn rebuild(&mut self, surface: vk::SurfaceKHR) -> Result<Swapchain, Error> {
        self.builder.previous_swapchain(self.swapchain);

        let new_swapchain = self.builder.build(Arc::clone(&self.device), surface)?;
        let old_swapchain = std::mem::replace(self, new_swapchain);

        Ok(old_swapchain)
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            let ext = self
                .device
                .extension::<ash::extensions::khr::Swapchain>()
                .unwrap();
            for view in &self.image_views {
                self.device.device().destroy_image_view(*view, None);
            }
            ext.destroy_swapchain(self.swapchain, None);
        }
    }
}
