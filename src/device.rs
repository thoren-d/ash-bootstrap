use std::{
    any::{Any, TypeId},
    collections::HashMap,
    os::raw::c_char,
    sync::Arc,
};

use ash::vk;

use crate::{util::streq, DeviceExtension, DeviceExtensionLoader, Error, Instance};

pub struct Device {
    instance: Arc<Instance>,
    device: ash::Device,
    physical_device: vk::PhysicalDevice,
    loaded_extensions: HashMap<TypeId, Box<dyn Any + 'static>>,
    graphics_queue: Option<(u32, vk::Queue)>,
    compute_queue: Option<(u32, vk::Queue)>,
    present_queue: Option<(u32, vk::Queue)>,
    transfer_queue: Option<(u32, vk::Queue)>,
}

pub struct DeviceBuilder {
    required_features: Option<Box<vk::PhysicalDeviceFeatures>>,
    optional_features: Option<Box<vk::PhysicalDeviceFeatures>>,
    required_extensions: Vec<(*const c_char, DeviceExtensionLoader)>,
    optional_extensions: Vec<(*const c_char, DeviceExtensionLoader)>,
    surface: Option<vk::SurfaceKHR>,
    preferred_device: Option<PreferredDevice>,
    needs_graphics: bool,
}

pub enum PreferredDevice {
    Chosen(u32),
    Discrete,
    Integrated,
}

impl DeviceBuilder {
    pub fn new() -> Self {
        DeviceBuilder {
            required_features: None,
            optional_features: None,
            required_extensions: Vec::new(),
            optional_extensions: Vec::new(),
            surface: None,
            preferred_device: None,
            needs_graphics: true,
        }
    }

    pub fn require_features(mut self, features: vk::PhysicalDeviceFeatures) -> Self {
        self.required_features = Some(Box::new(features));
        self
    }

    pub fn optional_features(mut self, features: vk::PhysicalDeviceFeatures) -> Self {
        self.optional_features = Some(Box::new(features));
        self
    }

    pub fn require_extension<E: DeviceExtension + 'static>(mut self) -> Self {
        if !self
            .required_extensions
            .iter()
            .any(|(name, _)| *name == E::name())
        {
            self.required_extensions
                .push((E::name(), Box::new(E::load)));
        }
        self
    }

    pub fn optional_extension<E: DeviceExtension + 'static>(mut self) -> Self {
        if !self
            .required_extensions
            .iter()
            .any(|(name, _)| *name == E::name())
            && !self
                .optional_extensions
                .iter()
                .any(|(name, _)| *name == E::name())
        {
            self.optional_extensions
                .push((E::name(), Box::new(E::load)));
        }
        self
    }

    pub fn surface(mut self, surface: vk::SurfaceKHR) -> Self {
        self.surface = Some(surface);
        self.require_extension::<ash::extensions::khr::Swapchain>()
    }

    pub fn graphics_optional(mut self) -> Self {
        self.needs_graphics = false;
        self
    }

    pub fn build(self, instance: Arc<Instance>) -> Result<Arc<Device>, Error> {
        unsafe {
            let physical_devices = instance.instance().enumerate_physical_devices()?;
            let physical_device = self.select_physical_device(&instance, &physical_devices)?;

            // Enable requested features if available.
            let mut enabled_features =
                if self.required_features.is_some() || self.optional_features.is_some() {
                    instance
                        .instance()
                        .get_physical_device_features(physical_device)
                } else {
                    Default::default()
                };
            if let Some(required_features) = self.required_features {
                enable_optional_features(&mut enabled_features, &required_features);
            }
            if let Some(optional_features) = self.optional_features {
                enable_optional_features(&mut enabled_features, &optional_features);
            }

            let mut requested_extensions: Vec<*const c_char> = Vec::new();
            // Check supported extensions. If there are no optional extensions,
            // we can skip querying extension support and just let device
            // creation fail.
            if !self.optional_extensions.is_empty() {
                let extensions = instance
                    .instance()
                    .enumerate_device_extension_properties(physical_device)?;
                for (name, _) in &self.optional_extensions {
                    for extension in &extensions {
                        if streq(*name, extension.extension_name.as_ptr()) {
                            requested_extensions.push(*name);
                            break;
                        }
                    }
                }
            }
            for (name, _) in &self.required_extensions {
                requested_extensions.push(*name);
            }

            let queue_families = instance
                .instance()
                .get_physical_device_queue_family_properties(physical_device);
            let graphics_queue = DeviceBuilder::find_graphics_queue(&queue_families);
            let compute_queue =
                DeviceBuilder::find_compute_queue(&queue_families).or(graphics_queue);
            let present_queue = self.surface.and_then(|surface| {
                DeviceBuilder::find_present_queue(
                    &instance,
                    physical_device,
                    surface,
                    &queue_families,
                )
                .unwrap_or_default()
            });
            let transfer_queue = DeviceBuilder::find_transfer_queue(&queue_families);

            let mut queue_families = Vec::<u32>::new();
            for qf in [graphics_queue, compute_queue, present_queue, transfer_queue]
                .into_iter()
                .flatten()
            {
                if !queue_families.contains(&qf) {
                    queue_families.push(qf)
                }
            }
            let queue_create_infos: Vec<vk::DeviceQueueCreateInfo> = queue_families
                .into_iter()
                .map(|qf| {
                    vk::DeviceQueueCreateInfo::builder()
                        .queue_family_index(qf)
                        .queue_priorities(&[1.0f32])
                        .build()
                })
                .collect();

            let create_info = vk::DeviceCreateInfo::builder()
                .enabled_extension_names(&requested_extensions)
                .enabled_features(&enabled_features)
                .queue_create_infos(&queue_create_infos);
            let device = instance
                .instance()
                .create_device(physical_device, &create_info, None)?;

            let mut loaded_extensions: HashMap<TypeId, Box<dyn Any + 'static>> = HashMap::new();
            for (name, loader) in self.optional_extensions {
                if requested_extensions.contains(&name) {
                    let ext = loader(instance.instance(), &device);
                    let id = ext.as_ref().type_id();
                    loaded_extensions.insert(id, ext);
                }
            }
            for (_, loader) in self.required_extensions {
                let ext = loader(instance.instance(), &device);
                let id = ext.as_ref().type_id();
                loaded_extensions.insert(id, ext);
            }

            let graphics_queue = graphics_queue.map(|qf| (qf, device.get_device_queue(qf, 0)));
            let compute_queue = compute_queue.map(|qf| (qf, device.get_device_queue(qf, 0)));
            let present_queue = present_queue.map(|qf| (qf, device.get_device_queue(qf, 0)));
            let transfer_queue = transfer_queue.map(|qf| (qf, device.get_device_queue(qf, 0)));

            Ok(Arc::new(Device {
                instance,
                device,
                physical_device,
                loaded_extensions,
                graphics_queue,
                compute_queue,
                present_queue,
                transfer_queue,
            }))
        }
    }

    unsafe fn select_physical_device(
        &self,
        instance: &Instance,
        physical_devices: &[vk::PhysicalDevice],
    ) -> Result<vk::PhysicalDevice, Error> {
        if let Some(preferred_device) = &self.preferred_device {
            match preferred_device {
                PreferredDevice::Chosen(idx) => {
                    let idx = *idx as usize;
                    if idx < physical_devices.len()
                        && self.is_device_suitable(instance, physical_devices[idx])?
                    {
                        return Ok(physical_devices[idx]);
                    }
                }
                PreferredDevice::Discrete => {
                    for &pd in physical_devices {
                        let props = instance.instance().get_physical_device_properties(pd);
                        if props.device_type == vk::PhysicalDeviceType::DISCRETE_GPU
                            && self.is_device_suitable(instance, pd)?
                        {
                            return Ok(pd);
                        }
                    }
                }
                PreferredDevice::Integrated => {
                    for &pd in physical_devices {
                        let props = instance.instance().get_physical_device_properties(pd);
                        if props.device_type == vk::PhysicalDeviceType::DISCRETE_GPU
                            && self.is_device_suitable(instance, pd)?
                        {
                            return Ok(pd);
                        }
                    }
                }
            }
        }

        // If there's no preference, just select the first suitable device.
        for &pd in physical_devices {
            if self.is_device_suitable(instance, pd)? {
                return Ok(pd);
            }
        }

        Err(Error::NoSuitableDevices)
    }

    unsafe fn is_device_suitable(
        &self,
        instance: &Instance,
        device: vk::PhysicalDevice,
    ) -> Result<bool, Error> {
        if let Some(required) = &self.required_features {
            let available_features = instance.instance().get_physical_device_features(device);

            if !has_required_features(&available_features, required.as_ref()) {
                return Ok(false);
            }
        }

        if !self.required_extensions.is_empty() {
            let available_extensions = instance
                .instance()
                .enumerate_device_extension_properties(device)?;

            for (req, _) in &self.required_extensions {
                let mut found = false;
                for ext in &available_extensions {
                    if streq(ext.extension_name.as_ptr(), *req) {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Ok(false);
                }
            }
        }

        let queue_families = instance
            .instance()
            .get_physical_device_queue_family_properties(device);

        if self.needs_graphics && DeviceBuilder::find_graphics_queue(&queue_families).is_none() {
            return Ok(false);
        }

        if let Some(surface) = self.surface {
            if DeviceBuilder::find_present_queue(instance, device, surface, &queue_families)?
                .is_none()
            {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn find_graphics_queue(queue_families: &[vk::QueueFamilyProperties]) -> Option<u32> {
        for (i, qf) in queue_families.iter().enumerate() {
            if qf
                .queue_flags
                .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
            {
                return Some(i as u32);
            }
        }

        None
    }

    unsafe fn find_present_queue(
        instance: &Instance,
        device: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        queue_families: &[vk::QueueFamilyProperties],
    ) -> Result<Option<u32>, Error> {
        let surface_ext = instance
            .extension::<ash::extensions::khr::Surface>()
            .unwrap();

        for i in 0..queue_families.len() {
            if surface_ext.get_physical_device_surface_support(device, i as u32, surface)? {
                return Ok(Some(i as u32));
            }
        }

        Ok(None)
    }

    fn find_transfer_queue(queue_families: &[vk::QueueFamilyProperties]) -> Option<u32> {
        for (i, qf) in queue_families.iter().enumerate() {
            if qf.queue_flags.contains(vk::QueueFlags::TRANSFER)
                && !qf
                    .queue_flags
                    .intersects(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
            {
                return Some(i as u32);
            }
        }

        None
    }

    fn find_compute_queue(queue_families: &[vk::QueueFamilyProperties]) -> Option<u32> {
        for (i, qf) in queue_families.iter().enumerate() {
            if qf.queue_flags.contains(vk::QueueFlags::COMPUTE)
                && !qf.queue_flags.contains(vk::QueueFlags::GRAPHICS)
            {
                return Some(i as u32);
            }
        }

        None
    }
}

impl Default for DeviceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Device {
    pub fn device(&self) -> &ash::Device {
        &self.device
    }

    pub fn instance(&self) -> &Instance {
        &self.instance
    }

    pub fn physical_device(&self) -> vk::PhysicalDevice {
        self.physical_device
    }

    pub fn extension<E: DeviceExtension + 'static>(&self) -> Option<&E> {
        let id = TypeId::of::<E>();
        self.loaded_extensions
            .get(&id)
            .map(|e| e.downcast_ref::<E>().unwrap())
    }

    pub fn graphics_queue(&self) -> Option<(u32, vk::Queue)> {
        self.graphics_queue
    }

    pub fn compute_queue(&self) -> Option<(u32, vk::Queue)> {
        self.compute_queue
    }

    pub fn present_queue(&self) -> Option<(u32, vk::Queue)> {
        self.present_queue
    }

    pub fn transfer_queue(&self) -> Option<(u32, vk::Queue)> {
        self.transfer_queue
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
        }
    }
}

macro_rules! check_required_feature {
    ($available:ident, $required:ident, $field:ident) => {
        if $required.$field != 0 && !$available.$field == 0 {
            return false;
        }
    };
}

fn has_required_features(
    available: &vk::PhysicalDeviceFeatures,
    required: &vk::PhysicalDeviceFeatures,
) -> bool {
    check_required_feature!(available, required, robust_buffer_access);
    check_required_feature!(available, required, full_draw_index_uint32);
    check_required_feature!(available, required, image_cube_array);
    check_required_feature!(available, required, independent_blend);
    check_required_feature!(available, required, geometry_shader);
    check_required_feature!(available, required, tessellation_shader);
    check_required_feature!(available, required, sample_rate_shading);
    check_required_feature!(available, required, dual_src_blend);
    check_required_feature!(available, required, logic_op);
    check_required_feature!(available, required, multi_draw_indirect);
    check_required_feature!(available, required, draw_indirect_first_instance);
    check_required_feature!(available, required, depth_clamp);
    check_required_feature!(available, required, depth_bias_clamp);
    check_required_feature!(available, required, fill_mode_non_solid);
    check_required_feature!(available, required, depth_bounds);
    check_required_feature!(available, required, wide_lines);
    check_required_feature!(available, required, large_points);
    check_required_feature!(available, required, alpha_to_one);
    check_required_feature!(available, required, multi_viewport);
    check_required_feature!(available, required, sampler_anisotropy);
    check_required_feature!(available, required, texture_compression_etc2);
    check_required_feature!(available, required, texture_compression_astc_ldr);
    check_required_feature!(available, required, texture_compression_bc);
    check_required_feature!(available, required, occlusion_query_precise);
    check_required_feature!(available, required, pipeline_statistics_query);
    check_required_feature!(available, required, vertex_pipeline_stores_and_atomics);
    check_required_feature!(available, required, fragment_stores_and_atomics);
    check_required_feature!(
        available,
        required,
        shader_tessellation_and_geometry_point_size
    );
    check_required_feature!(available, required, shader_image_gather_extended);
    check_required_feature!(available, required, shader_storage_image_extended_formats);
    check_required_feature!(available, required, shader_storage_image_multisample);
    check_required_feature!(
        available,
        required,
        shader_storage_image_read_without_format
    );
    check_required_feature!(
        available,
        required,
        shader_storage_image_write_without_format
    );
    check_required_feature!(
        available,
        required,
        shader_uniform_buffer_array_dynamic_indexing
    );
    check_required_feature!(
        available,
        required,
        shader_sampled_image_array_dynamic_indexing
    );
    check_required_feature!(
        available,
        required,
        shader_storage_buffer_array_dynamic_indexing
    );
    check_required_feature!(
        available,
        required,
        shader_storage_image_array_dynamic_indexing
    );
    check_required_feature!(available, required, shader_clip_distance);
    check_required_feature!(available, required, shader_cull_distance);
    check_required_feature!(available, required, shader_float64);
    check_required_feature!(available, required, shader_int64);
    check_required_feature!(available, required, shader_int16);
    check_required_feature!(available, required, shader_resource_residency);
    check_required_feature!(available, required, shader_resource_min_lod);
    check_required_feature!(available, required, sparse_binding);
    check_required_feature!(available, required, sparse_residency_buffer);
    check_required_feature!(available, required, sparse_residency_image2_d);
    check_required_feature!(available, required, sparse_residency_image3_d);
    check_required_feature!(available, required, sparse_residency2_samples);
    check_required_feature!(available, required, sparse_residency4_samples);
    check_required_feature!(available, required, sparse_residency8_samples);
    check_required_feature!(available, required, sparse_residency16_samples);
    check_required_feature!(available, required, sparse_residency_aliased);
    check_required_feature!(available, required, variable_multisample_rate);
    check_required_feature!(available, required, inherited_queries);

    true
}

macro_rules! maybe_enable_feature {
    ($available:ident, $optional:ident, $field:ident) => {
        $available.$field = if $available.$field != 0 {
            $optional.$field
        } else {
            0
        }
    };
}

fn enable_optional_features(
    available: &mut vk::PhysicalDeviceFeatures,
    optional: &vk::PhysicalDeviceFeatures,
) {
    maybe_enable_feature!(available, optional, robust_buffer_access);
    maybe_enable_feature!(available, optional, full_draw_index_uint32);
    maybe_enable_feature!(available, optional, image_cube_array);
    maybe_enable_feature!(available, optional, independent_blend);
    maybe_enable_feature!(available, optional, geometry_shader);
    maybe_enable_feature!(available, optional, tessellation_shader);
    maybe_enable_feature!(available, optional, sample_rate_shading);
    maybe_enable_feature!(available, optional, dual_src_blend);
    maybe_enable_feature!(available, optional, logic_op);
    maybe_enable_feature!(available, optional, multi_draw_indirect);
    maybe_enable_feature!(available, optional, draw_indirect_first_instance);
    maybe_enable_feature!(available, optional, depth_clamp);
    maybe_enable_feature!(available, optional, depth_bias_clamp);
    maybe_enable_feature!(available, optional, fill_mode_non_solid);
    maybe_enable_feature!(available, optional, depth_bounds);
    maybe_enable_feature!(available, optional, wide_lines);
    maybe_enable_feature!(available, optional, large_points);
    maybe_enable_feature!(available, optional, alpha_to_one);
    maybe_enable_feature!(available, optional, multi_viewport);
    maybe_enable_feature!(available, optional, sampler_anisotropy);
    maybe_enable_feature!(available, optional, texture_compression_etc2);
    maybe_enable_feature!(available, optional, texture_compression_astc_ldr);
    maybe_enable_feature!(available, optional, texture_compression_bc);
    maybe_enable_feature!(available, optional, occlusion_query_precise);
    maybe_enable_feature!(available, optional, pipeline_statistics_query);
    maybe_enable_feature!(available, optional, vertex_pipeline_stores_and_atomics);
    maybe_enable_feature!(available, optional, fragment_stores_and_atomics);
    maybe_enable_feature!(
        available,
        optional,
        shader_tessellation_and_geometry_point_size
    );
    maybe_enable_feature!(available, optional, shader_image_gather_extended);
    maybe_enable_feature!(available, optional, shader_storage_image_extended_formats);
    maybe_enable_feature!(available, optional, shader_storage_image_multisample);
    maybe_enable_feature!(
        available,
        optional,
        shader_storage_image_read_without_format
    );
    maybe_enable_feature!(
        available,
        optional,
        shader_storage_image_write_without_format
    );
    maybe_enable_feature!(
        available,
        optional,
        shader_uniform_buffer_array_dynamic_indexing
    );
    maybe_enable_feature!(
        available,
        optional,
        shader_sampled_image_array_dynamic_indexing
    );
    maybe_enable_feature!(
        available,
        optional,
        shader_storage_buffer_array_dynamic_indexing
    );
    maybe_enable_feature!(
        available,
        optional,
        shader_storage_image_array_dynamic_indexing
    );
    maybe_enable_feature!(available, optional, shader_clip_distance);
    maybe_enable_feature!(available, optional, shader_cull_distance);
    maybe_enable_feature!(available, optional, shader_float64);
    maybe_enable_feature!(available, optional, shader_int64);
    maybe_enable_feature!(available, optional, shader_int16);
    maybe_enable_feature!(available, optional, shader_resource_residency);
    maybe_enable_feature!(available, optional, shader_resource_min_lod);
    maybe_enable_feature!(available, optional, sparse_binding);
    maybe_enable_feature!(available, optional, sparse_residency_buffer);
    maybe_enable_feature!(available, optional, sparse_residency_image2_d);
    maybe_enable_feature!(available, optional, sparse_residency_image3_d);
    maybe_enable_feature!(available, optional, sparse_residency2_samples);
    maybe_enable_feature!(available, optional, sparse_residency4_samples);
    maybe_enable_feature!(available, optional, sparse_residency8_samples);
    maybe_enable_feature!(available, optional, sparse_residency16_samples);
    maybe_enable_feature!(available, optional, sparse_residency_aliased);
    maybe_enable_feature!(available, optional, variable_multisample_rate);
    maybe_enable_feature!(available, optional, inherited_queries);
}
