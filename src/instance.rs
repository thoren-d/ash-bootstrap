use std::{
    any::{Any, TypeId},
    collections::HashMap,
    ffi::{c_void, CStr, CString},
    os::raw::c_char,
    sync::Arc,
};

use ash::{vk, Entry};

use crate::{util::streq, Error, InstanceExtension, InstanceExtensionLoader};

pub struct Instance {
    entry: Entry,
    instance: ash::Instance,
    loaded_extensions: HashMap<TypeId, Box<dyn Any + 'static>>,
}

pub struct InstanceBuilder<'a> {
    api_version: u32,
    app_name: &'a str,
    engine_name: &'a str,
    app_version: u32,
    engine_version: u32,
    required_extensions: Vec<(*const c_char, InstanceExtensionLoader)>,
    optional_extensions: Vec<(*const c_char, InstanceExtensionLoader)>,
    enabled_layers: Vec<*const c_char>,
    debug_messenger_fn: vk::PFN_vkDebugUtilsMessengerCallbackEXT,
    is_headless: bool,
}

impl<'a> InstanceBuilder<'a> {
    pub fn new() -> Self {
        InstanceBuilder {
            api_version: vk::API_VERSION_1_0,
            app_name: "unspecified",
            engine_name: "unspecified",
            app_version: 0,
            engine_version: 0,
            required_extensions: Vec::default(),
            optional_extensions: Vec::default(),
            enabled_layers: Vec::default(),
            debug_messenger_fn: None,
            is_headless: false,
        }
    }

    pub fn api_version(mut self, version: u32) -> Self {
        self.api_version = version;
        self
    }

    pub fn app_name(mut self, name: &'a str) -> Self {
        self.app_name = name;
        self
    }

    pub fn engine_name(mut self, name: &'a str) -> Self {
        self.engine_name = name;
        self
    }

    pub fn app_version(mut self, version: u32) -> Self {
        self.app_version = version;
        self
    }

    pub fn engine_version(mut self, version: u32) -> Self {
        self.engine_version = version;
        self
    }

    /// Create an instance without surface support.
    pub fn headless(mut self) -> Self {
        self.is_headless = true;
        self
    }

    pub fn require_extension<E: InstanceExtension + 'static>(mut self) -> Self {
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

    pub fn optional_extension<E: InstanceExtension + 'static>(mut self) -> Self {
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

    pub fn use_default_debug_messenger(mut self) -> Self {
        self.debug_messenger_fn = Some(default_debug_message_func);
        self.require_extension::<ash::extensions::ext::DebugUtils>()
    }

    pub fn request_validation_layers(mut self) -> Self {
        self.enabled_layers
            .push(b"VK_LAYER_KHRONOS_validation\0".as_ptr() as *const c_char);
        self
    }

    pub fn build(mut self) -> Result<Arc<Instance>, Error> {
        unsafe {
            self = if !self.is_headless {
                self.require_surface_extensions()
            } else {
                self
            };

            let entry = Entry::load()?;

            let mut requested_extensions: Vec<*const c_char> = Vec::new();
            // Check supported extensions. If there are no optional extensions,
            // we can skip querying extension support and just let instance
            // creation fail.
            if !self.optional_extensions.is_empty() {
                let extensions = entry.enumerate_instance_extension_properties(None)?;
                for (name, _) in &self.optional_extensions {
                    for extension in &extensions {
                        if streq(*name, extension.extension_name.as_ptr()) {
                            requested_extensions.push(*name);
                            break;
                        }
                    }
                }
            }

            // Add the required extensions
            for (name, _) in &self.required_extensions {
                requested_extensions.push(*name);
            }

            let app_name = CString::new(self.app_name).unwrap();
            let engine_name = CString::new(self.engine_name).unwrap();

            let app_info = vk::ApplicationInfo::builder()
                .api_version(self.api_version)
                .application_name(app_name.as_c_str())
                .application_version(self.app_version)
                .engine_name(engine_name.as_c_str())
                .engine_version(self.engine_version);

            let create_info = vk::InstanceCreateInfo::builder()
                .application_info(&app_info)
                .enabled_extension_names(&requested_extensions)
                .enabled_layer_names(&self.enabled_layers);

            let instance = match self.debug_messenger_fn {
                Some(func) => {
                    let mut debug_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
                        .message_severity(
                            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                                | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                                | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE,
                        )
                        .message_type(
                            vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                                | vk::DebugUtilsMessageTypeFlagsEXT::GENERAL,
                        )
                        .pfn_user_callback(Some(func));
                    let create_info = create_info.push_next(&mut debug_info);

                    entry.create_instance(&create_info, None)?
                }
                None => entry.create_instance(&create_info, None)?,
            };

            let mut loaded_extensions: HashMap<TypeId, Box<dyn Any + 'static>> = HashMap::new();
            for (name, loader) in self.optional_extensions {
                if requested_extensions.contains(&name) {
                    let ext = loader(&entry, &instance);
                    let id = ext.as_ref().type_id();
                    loaded_extensions.insert(id, ext);
                }
            }

            for (_, loader) in self.required_extensions {
                let ext = loader(&entry, &instance);
                let id = ext.as_ref().type_id();
                loaded_extensions.insert(id, ext);
            }

            Ok(Arc::new(Instance {
                entry,
                instance,
                loaded_extensions,
            }))
        }
    }

    fn require_surface_extensions(mut self) -> Self {
        self = self.require_extension::<ash::extensions::khr::Surface>();
        if cfg!(target_os = "windows") {
            self.require_extension::<ash::extensions::khr::Win32Surface>()
        } else if cfg!(target_os = "linux") {
            self.optional_extension::<ash::extensions::khr::XlibSurface>()
                .optional_extension::<ash::extensions::khr::WaylandSurface>()
                .optional_extension::<ash::extensions::khr::XcbSurface>()
        } else if cfg!(target_os = "android") {
            self.require_extension::<ash::extensions::khr::AndroidSurface>()
        } else if cfg!(target_os = "macos") {
            self.require_extension::<ash::extensions::mvk::MacOSSurface>()
        } else if cfg!(target_os = "ios") {
            self.require_extension::<ash::extensions::mvk::IOSSurface>()
        } else {
            panic!("Target OS has no surface extension (yet).");
        }
    }
}

impl<'a> Default for InstanceBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl Instance {
    pub fn entry(&self) -> &Entry {
        &self.entry
    }

    pub fn instance(&self) -> &ash::Instance {
        &self.instance
    }

    pub fn extension<E: InstanceExtension + 'static>(&self) -> Option<&E> {
        let id = TypeId::of::<E>();
        self.loaded_extensions
            .get(&id)
            .map(|e| e.downcast_ref::<E>().unwrap())
    }

    #[cfg(feature = "window")]
    pub fn create_surface<W: raw_window_handle::HasRawWindowHandle>(
        &self,
        window: &W,
    ) -> Result<vk::SurfaceKHR, Error> {
        use ash::vk::{
            WaylandSurfaceCreateInfoKHR, Win32SurfaceCreateInfoKHR, XcbSurfaceCreateInfoKHR,
            XlibSurfaceCreateInfoKHR,
        };
        use raw_window_handle::RawWindowHandle;

        match window.raw_window_handle() {
            RawWindowHandle::Win32(handle) => {
                let ext = self
                    .extension::<ash::extensions::khr::Win32Surface>()
                    .expect("VK_KHR_win32_surface not loaded.");
                let create_info = Win32SurfaceCreateInfoKHR::builder()
                    .hinstance(handle.hinstance)
                    .hwnd(handle.hwnd);
                unsafe { Ok(ext.create_win32_surface(&create_info, None)?) }
            }
            RawWindowHandle::Xlib(handle) => {
                let ext = self
                    .extension::<ash::extensions::khr::XlibSurface>()
                    .expect("VK_KHR_xlib_surface not loaded.");
                let create_info = XlibSurfaceCreateInfoKHR::builder()
                    .dpy(handle.display as _)
                    .window(handle.window);
                unsafe { Ok(ext.create_xlib_surface(&create_info, None)?) }
            }
            RawWindowHandle::Xcb(handle) => {
                let ext = self
                    .extension::<ash::extensions::khr::XcbSurface>()
                    .expect("VK_KHR_xcb_surface not loaded.");
                let create_info = XcbSurfaceCreateInfoKHR::builder()
                    .connection(handle.connection)
                    .window(handle.window);
                unsafe { Ok(ext.create_xcb_surface(&create_info, None)?) }
            }
            RawWindowHandle::Wayland(handle) => {
                let ext = self
                    .extension::<ash::extensions::khr::WaylandSurface>()
                    .expect("VK_KHR_wayland_surface not loaded.");
                let create_info = WaylandSurfaceCreateInfoKHR::builder()
                    .display(handle.display)
                    .surface(handle.surface);
                unsafe { Ok(ext.create_wayland_surface(&create_info, None)?) }
            }
            _ => {
                unimplemented!("Support for this window system isn't done yet.");
            }
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            self.instance.destroy_instance(None);
        }
    }
}

unsafe extern "system" fn default_debug_message_func(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {
    use crate::util::{error, info, trace, warn};
    let msg = CStr::from_ptr((*p_callback_data).p_message).to_string_lossy();
    match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => {
            error!(target: "vulkan", "[{:?}]: {}", message_types, msg)
        }
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => {
            warn!(target: "vulkan", "[{:?}]: {}", message_types, msg)
        }
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => {
            info!(target: "vulkan", "[{:?}]: {}", message_types, msg)
        }
        _ => trace!(target: "vulkan", "[{:?}]: {}", message_types, msg),
    };
    vk::Bool32::from(true)
}
