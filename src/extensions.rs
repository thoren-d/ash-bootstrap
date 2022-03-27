use std::os::raw::c_char;

use ash::{Device, Entry, Instance};

pub trait InstanceExtension {
    fn name() -> *const c_char;
    fn load(entry: &Entry, instance: &Instance) -> Box<dyn std::any::Any + 'static>;
}
pub type InstanceExtensionLoader =
    Box<dyn FnOnce(&ash::Entry, &ash::Instance) -> Box<dyn std::any::Any + 'static>>;

pub trait DeviceExtension {
    fn name() -> *const c_char;
    fn load(instance: &Instance, device: &Device) -> Box<dyn std::any::Any + 'static>;
}
pub type DeviceExtensionLoader =
    Box<dyn FnOnce(&ash::Instance, &ash::Device) -> Box<dyn std::any::Any + 'static>>;

macro_rules! impl_instance_extension {
    ($ext:ty) => {
        impl InstanceExtension for $ext {
            fn name() -> *const c_char {
                Self::name().as_ptr()
            }

            fn load(entry: &Entry, instance: &Instance) -> Box<dyn std::any::Any + 'static> {
                Box::new(Self::new(entry, instance))
            }
        }
    };
}

macro_rules! impl_device_extension {
    ($ext:ty) => {
        impl DeviceExtension for $ext {
            fn name() -> *const c_char {
                Self::name().as_ptr()
            }

            fn load(instance: &Instance, device: &Device) -> Box<dyn std::any::Any + 'static> {
                Box::new(Self::new(instance, device))
            }
        }
    };
}

// ext
impl_device_extension!(ash::extensions::ext::BufferDeviceAddress);
impl_instance_extension!(ash::extensions::ext::CalibratedTimestamps);
impl_instance_extension!(ash::extensions::ext::DebugUtils);
impl_device_extension!(ash::extensions::ext::ExtendedDynamicState);
impl_device_extension!(ash::extensions::ext::ExtendedDynamicState2);
impl_device_extension!(ash::extensions::ext::FullScreenExclusive);
impl_instance_extension!(ash::extensions::ext::MetalSurface);
impl_device_extension!(ash::extensions::ext::PrivateData);
impl_instance_extension!(ash::extensions::ext::ToolingInfo);

// khr
impl_device_extension!(ash::extensions::khr::AccelerationStructure);
impl_instance_extension!(ash::extensions::khr::AndroidSurface);
impl_device_extension!(ash::extensions::khr::BufferDeviceAddress);
impl_device_extension!(ash::extensions::khr::CopyCommands2);
impl_device_extension!(ash::extensions::khr::CreateRenderPass2);
impl_device_extension!(ash::extensions::khr::DeferredHostOperations);
impl_instance_extension!(ash::extensions::khr::Display);
impl_device_extension!(ash::extensions::khr::DisplaySwapchain);
impl_device_extension!(ash::extensions::khr::DrawIndirectCount);
impl_device_extension!(ash::extensions::khr::DynamicRendering);
impl_device_extension!(ash::extensions::khr::ExternalFenceFd);
impl_device_extension!(ash::extensions::khr::ExternalFenceWin32);
impl_device_extension!(ash::extensions::khr::ExternalMemoryFd);
impl_device_extension!(ash::extensions::khr::ExternalMemoryWin32);
impl_device_extension!(ash::extensions::khr::ExternalSemaphoreFd);
impl_device_extension!(ash::extensions::khr::ExternalSemaphoreWin32);
impl_device_extension!(ash::extensions::khr::GetMemoryRequirements2);
impl_instance_extension!(ash::extensions::khr::GetPhysicalDeviceProperties2);
impl_instance_extension!(ash::extensions::khr::GetSurfaceCapabilities2);
impl_device_extension!(ash::extensions::khr::Maintenance1);
impl_device_extension!(ash::extensions::khr::Maintenance3);
impl_device_extension!(ash::extensions::khr::Maintenance4);
impl_device_extension!(ash::extensions::khr::PipelineExecutableProperties);
impl_device_extension!(ash::extensions::khr::PresentWait);
impl_device_extension!(ash::extensions::khr::PushDescriptor);
impl_device_extension!(ash::extensions::khr::RayTracingPipeline);
impl_instance_extension!(ash::extensions::khr::Surface);
impl_device_extension!(ash::extensions::khr::Swapchain);
impl_device_extension!(ash::extensions::khr::Synchronization2);
impl_device_extension!(ash::extensions::khr::TimelineSemaphore);
impl_instance_extension!(ash::extensions::khr::WaylandSurface);
impl_instance_extension!(ash::extensions::khr::Win32Surface);
impl_instance_extension!(ash::extensions::khr::XcbSurface);
impl_instance_extension!(ash::extensions::khr::XlibSurface);

// mvk
impl_instance_extension!(ash::extensions::mvk::IOSSurface);
impl_instance_extension!(ash::extensions::mvk::MacOSSurface);

// nn
impl_instance_extension!(ash::extensions::nn::ViSurface);

// nv
impl_device_extension!(ash::extensions::nv::DeviceDiagnosticCheckpoints);
impl_device_extension!(ash::extensions::nv::MeshShader);
impl_device_extension!(ash::extensions::nv::RayTracing);

impl InstanceExtension for ash::extensions::ext::PhysicalDeviceDrm {
    fn name() -> *const c_char {
        Self::name().as_ptr()
    }

    fn load(_: &Entry, _: &Instance) -> Box<dyn std::any::Any> {
        Box::new(Self)
    }
}
