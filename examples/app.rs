use std::sync::Arc;

use ash::vk;
use ash_bootstrap::{DeviceBuilder, InstanceBuilder, SwapchainBuilder};
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    simple_logger::SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(1920, 1080))
        .build(&event_loop)
        .unwrap();

    let instance = InstanceBuilder::new()
        .app_name("Hello ash-bootstrap")
        .app_version(1)
        .use_default_debug_messenger()
        .request_validation_layers()
        .build()
        .expect("Failed to create instance.");

    let surface = instance
        .create_surface(&window)
        .expect("Failed to create surface.");

    let device = DeviceBuilder::new()
        .surface(surface)
        .optional_features(
            vk::PhysicalDeviceFeatures::builder()
                .texture_compression_bc(true)
                .sample_rate_shading(true)
                .build(),
        )
        .build(Arc::clone(&instance))
        .expect("Failed to create device.");

    let graphics_queue = device.graphics_queue().expect("No graphics queue?");
    println!("{graphics_queue:?}");

    let swapchain = SwapchainBuilder::new()
        .build(Arc::clone(&device), surface)
        .expect("Failed to create swapchain.");
    println!(
        "Format {:?}, Extent {:?}",
        swapchain.format(),
        swapchain.extent()
    );

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}
