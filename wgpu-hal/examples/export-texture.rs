//! Test creating an exportable Vulkan texture and exporting its memory as a
//! DMA-BUF file descriptor. This is a headless (no window) example.
//!
//! Run with:
//! ```sh
//! cargo run --example export-texture --features vulkan
//! ```

extern crate wgpu_hal as hal;

use hal::{Adapter as _, Device as _, Instance as _};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== wgpu-hal Vulkan texture export test ===\n");

    // 1. Create a headless Vulkan instance (no surface).
    let instance_desc = hal::InstanceDescriptor {
        name: "export-texture-test",
        flags: wgpu_types::InstanceFlags::from_build_config().with_env(),
        memory_budget_thresholds: wgpu_types::MemoryBudgetThresholds::default(),
        backend_options: wgpu_types::BackendOptions::default(),
        telemetry: None,
    };
    let instance =
        unsafe { <hal::api::Vulkan as hal::Api>::Instance::init(&instance_desc)? };

    // 2. Enumerate adapters (without a surface — headless).
    let adapters = unsafe { instance.enumerate_adapters(None) };
    if adapters.is_empty() {
        return Err("No Vulkan adapters found".into());
    }

    let exposed = &adapters[0];
    println!(
        "Adapter: {} ({:?})",
        exposed.info.name, exposed.info.device_type
    );
    println!(
        "DMA-BUF export support: {}",
        exposed
            .features
            .contains(wgpu_types::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF)
    );

    if !exposed
        .features
        .contains(wgpu_types::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF)
    {
        println!("\nSkipping: adapter does not support VULKAN_EXTERNAL_MEMORY_DMA_BUF");
        return Ok(());
    }

    // 3. Open device with the DMA-BUF feature enabled.
    let hal::OpenDevice { device, queue: _ } = unsafe {
        exposed.adapter.open(
            wgpu_types::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF,
            &wgpu_types::Limits::default(),
            &wgpu_types::MemoryHints::default(),
        )?
    };

    // List the enabled extensions for confirmation.
    println!(
        "Enabled extensions: {:?}\n",
        device
            .enabled_device_extensions()
            .iter()
            .filter_map(|e| e.to_str().ok())
            .collect::<Vec<_>>()
    );

    // 4. Test texture formats — NV12 (8-bit) and P010-equivalent (16-bit).
    let test_cases: &[(wgpu_types::TextureFormat, u32, u32, &str)] = &[
        (wgpu_types::TextureFormat::R8Unorm, 1920, 1080, "R8Unorm 1920x1080 (Y plane, 8-bit)"),
        (wgpu_types::TextureFormat::Rg8Unorm, 960, 540, "Rg8Unorm 960x540 (UV plane, 8-bit)"),
        (wgpu_types::TextureFormat::R16Unorm, 1920, 1080, "R16Unorm 1920x1080 (Y plane, 10-bit)"),
        (wgpu_types::TextureFormat::Rg16Unorm, 960, 540, "Rg16Unorm 960x540 (UV plane, 10-bit)"),
        (wgpu_types::TextureFormat::Rgba8Unorm, 1920, 1080, "Rgba8Unorm 1920x1080 (color)"),
    ];

    for (format, width, height, label) in test_cases {
        println!("--- {label} ---");

        let desc = hal::TextureDescriptor {
            label: Some(label),
            size: wgpu_types::Extent3d {
                width: *width,
                height: *height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu_types::TextureDimension::D2,
            format: *format,
            usage: wgpu_types::TextureUses::COPY_DST | wgpu_types::TextureUses::RESOURCE,
            memory_flags: hal::MemoryFlags::empty(),
            view_formats: vec![],
        };

        // Create exportable texture.
        let texture = match unsafe { device.create_exportable_texture(&desc) } {
            Ok(t) => t,
            Err(e) => {
                println!("  FAILED to create: {e:?}\n");
                continue;
            }
        };
        println!("  Created exportable texture OK");

        // Export the FD.
        match unsafe { device.export_texture_memory_fd(&texture) } {
            Ok(exported) => {
                println!("  Exported FD: {}", exported.fd);
                println!("  Allocation size: {} bytes", exported.size);
                println!("  Row pitch: {} bytes", exported.row_pitch);

                // Close the FD since we're just testing.
                unsafe { libc::close(exported.fd) };
                println!("  Closed FD OK");
            }
            Err(e) => {
                println!("  FAILED to export: {e:?}");
            }
        }

        // Clean up.
        unsafe { device.destroy_texture(texture) };
        println!();
    }

    println!("=== Done ===");
    Ok(())
}
