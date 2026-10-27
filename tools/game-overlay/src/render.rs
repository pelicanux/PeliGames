use crate::tools::Tools;
use ash::{vk, Device, Instance};

/// Precision does not determine the transfer function: packed 10-bit UNORM
/// with SRGB_NONLINEAR is SDR too. HDR color spaces are rejected by the layer.
pub(crate) fn sdr_format(format: vk::Format) -> bool {
    matches!(
        format,
        vk::Format::B8G8R8A8_UNORM
            | vk::Format::R8G8B8A8_UNORM
            | vk::Format::B8G8R8A8_SRGB
            | vk::Format::R8G8B8A8_SRGB
            | vk::Format::A2B10G10R10_UNORM_PACK32
            | vk::Format::A2R10G10B10_UNORM_PACK32
    )
}

/// Bounded rotating frame slots. Present semaphores are per swapchain
/// image: reacquisition, not the submit fence, establishes safe semaphore reuse.
pub struct Render {
    device: Device,
    pool: vk::CommandPool,
    commands: Vec<vk::CommandBuffer>,
    fences: Vec<vk::Fence>,
    frame: usize,
    pass: vk::RenderPass,
    views: Vec<vk::ImageView>,
    buffers: Vec<vk::Framebuffer>,
    semaphores: Vec<vk::Semaphore>,
    renderer: Option<egui_ash_renderer::Renderer>,
    context: egui::Context,
    extent: vk::Extent2D,
    family: u32,
    free_textures: Vec<egui::TextureId>,
    pub failed_submit: Option<vk::Result>,
}
impl Render {
    pub fn family(&self) -> u32 {
        self.family
    }
    pub unsafe fn new(
        instance: &Instance,
        physical: vk::PhysicalDevice,
        device: Device,
        format: vk::Format,
        extent: vk::Extent2D,
        images: &[vk::Image],
        family: u32,
    ) -> Result<Self, String> {
        if !sdr_format(format) {
            return Err("Formato de imagem SDR ainda não suportado pelo overlay.".into());
        }
        let features = instance.get_physical_device_format_properties(physical, format);
        if !features.optimal_tiling_features.contains(
            vk::FormatFeatureFlags::COLOR_ATTACHMENT
                | vk::FormatFeatureFlags::COLOR_ATTACHMENT_BLEND,
        ) {
            return Err(
                "O driver não permite composição com transparência neste formato SDR.".into(),
            );
        }
        let context = egui::Context::default();
        context.set_visuals(egui::Visuals::dark());
        context.style_mut(|style| {
            for font in style.text_styles.values_mut() {
                font.size *= 1.2;
            }
        });
        let mut out = Self {
            device,
            pool: vk::CommandPool::null(),
            commands: Vec::new(),
            fences: Vec::new(),
            frame: 0,
            pass: vk::RenderPass::null(),
            views: Vec::new(),
            buffers: Vec::new(),
            semaphores: Vec::new(),
            renderer: None,
            context,
            extent,
            family,
            free_textures: Vec::new(),
            failed_submit: None,
        };
        let err = |e: vk::Result| format!("Vulkan overlay: {e:?}");
        out.pool = out
            .device
            .create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(family)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
            .map_err(err)?;
        out.commands = out
            .device
            .allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(out.pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(images.len() as u32),
            )
            .map_err(err)?;
        for _ in images {
            out.fences.push(
                out.device
                    .create_fence(
                        &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                        None,
                    )
                    .map_err(err)?,
            );
        }
        let attachments = [vk::AttachmentDescription::default()
            .format(format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::LOAD)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];
        let color = [vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
        let subpasses = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color)];
        let dependencies = [vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::ALL_COMMANDS)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::MEMORY_WRITE)
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            )];
        out.pass = out
            .device
            .create_render_pass(
                &vk::RenderPassCreateInfo::default()
                    .attachments(&attachments)
                    .subpasses(&subpasses)
                    .dependencies(&dependencies),
                None,
            )
            .map_err(err)?;
        for image in images {
            let view = out
                .device
                .create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(*image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(format)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .level_count(1)
                                .layer_count(1),
                        ),
                    None,
                )
                .map_err(err)?;
            out.views.push(view);
            let views = [view];
            out.buffers.push(
                out.device
                    .create_framebuffer(
                        &vk::FramebufferCreateInfo::default()
                            .render_pass(out.pass)
                            .attachments(&views)
                            .width(extent.width)
                            .height(extent.height)
                            .layers(1),
                        None,
                    )
                    .map_err(err)?,
            );
            out.semaphores.push(
                out.device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                    .map_err(err)?,
            );
        }
        out.renderer = Some(
            egui_ash_renderer::Renderer::with_default_allocator(
                instance,
                physical,
                out.device.clone(),
                out.pass,
                egui_ash_renderer::Options {
                    in_flight_frames: images.len(),
                    srgb_framebuffer: matches!(
                        format,
                        vk::Format::B8G8R8A8_SRGB | vk::Format::R8G8B8A8_SRGB
                    ),
                    ..Default::default()
                },
            )
            .map_err(|e| e.to_string())?,
        );
        Ok(out)
    }

    /// Returns a semaphore only after a successful submit. On pre-submit
    /// failure the caller must forward the application's original wait list.
    pub unsafe fn paint(
        &mut self,
        queue: vk::Queue,
        family: u32,
        index: usize,
        waits: &[vk::Semaphore],
        controller: &Tools,
    ) -> Result<Option<vk::Semaphore>, String> {
        if family != self.family || index >= self.buffers.len() {
            return Ok(None);
        }
        let fence = self.fences[self.frame];
        let command = self.commands[self.frame];
        match self.device.get_fence_status(fence) {
            Ok(true) => {}
            Ok(false) => return Ok(None),
            Err(e) => return Err(format!("{e:?}")),
        }
        crate::wayland_cursor::reconcile();
        let raw = {
            let Ok(mut input) = crate::state().lock() else {
                return Ok(None);
            };
            input.extent = egui::vec2(self.extent.width as f32, self.extent.height as f32);
            input.ready = crate::input_available();
            if !input.ready || !input.visible {
                return Ok(None);
            }
            input.frame(&self.context, crate::uptime())
        };
        let output = self.context.run(raw, |ctx| {
            controller.draw(ctx, egui::vec2(self.extent.width as f32, self.extent.height as f32));
            // Both adapters hide the native cursor before drawing the same
            // panel cursor. A failed lease must never create a second arrow.
            let cursor_hidden = match crate::target() {
                crate::Target::Xlib(window) => crate::x11::cursor_hidden(window),
                crate::Target::Wayland(_) => crate::wayland_cursor::hidden(),
                _ => false,
            };
            if cursor_hidden {
                if let Ok(input) = crate::state().lock() {
                    if !input.capturing() { return; }
                    let p = input.position;
                    ctx.layer_painter(egui::LayerId::new(
                        egui::Order::Tooltip,
                        egui::Id::new("peli-cursor"),
                    ))
                    .add(egui::Shape::convex_polygon(
                        vec![
                            p,
                            p + egui::vec2(0., 16.),
                            p + egui::vec2(5., 11.),
                            p + egui::vec2(11., 11.),
                        ],
                        egui::Color32::WHITE,
                        egui::Stroke::new(1.0_f32, egui::Color32::BLACK),
                    ));
                }
            }
        });
        let renderer = self.renderer.as_mut().ok_or("Renderer ausente")?;
        // Texture changes are rare (font atlas updates). All readers must finish
        // before modifying/freeing a shared texture; ordinary frames do not wait.
        if !output.textures_delta.set.is_empty() {
            self.device
                .wait_for_fences(&self.fences, true, u64::MAX)
                .map_err(|e| format!("{e:?}"))?;
        }
        if self
            .fences
            .iter()
            .all(|f| self.device.get_fence_status(*f) == Ok(true))
        {
            renderer
                .free_textures(&self.free_textures)
                .map_err(|e| e.to_string())?;
            self.free_textures.clear();
        }
        renderer
            .set_textures(queue, self.pool, &output.textures_delta.set)
            .map_err(|e| e.to_string())?;
        let primitives = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        if primitives.is_empty() {
            return Ok(None);
        }
        self.device
            .reset_command_buffer(command, vk::CommandBufferResetFlags::empty())
            .map_err(|e| format!("{e:?}"))?;
        self.device
            .begin_command_buffer(
                command,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
            .map_err(|e| format!("{e:?}"))?;
        self.device.cmd_begin_render_pass(
            command,
            &vk::RenderPassBeginInfo::default()
                .render_pass(self.pass)
                .framebuffer(self.buffers[index])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.extent,
                }),
            vk::SubpassContents::INLINE,
        );
        renderer
            .cmd_draw(command, self.extent, output.pixels_per_point, &primitives)
            .map_err(|e| e.to_string())?;
        self.device.cmd_end_render_pass(command);
        self.device
            .end_command_buffer(command)
            .map_err(|e| format!("{e:?}"))?;
        let stages = vec![vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT; waits.len()];
        let commands = [command];
        let signals = [self.semaphores[index]];
        let submit = [vk::SubmitInfo::default()
            .wait_semaphores(waits)
            .wait_dst_stage_mask(&stages)
            .command_buffers(&commands)
            .signal_semaphores(&signals)];
        self.device
            .reset_fences(&[fence])
            .map_err(|e| format!("{e:?}"))?;
        if let Err(error) = self.device.queue_submit(queue, &submit, fence) {
            self.failed_submit = Some(error);
            return Err(format!("{error:?}"));
        }
        self.free_textures.extend(output.textures_delta.free);
        crate::drawn();
        self.frame = (self.frame + 1) % self.fences.len();
        Ok(Some(signals[0]))
    }
}
impl Drop for Render {
    fn drop(&mut self) {
        unsafe {
            // Destruction is a lifecycle boundary, never a per-frame wait. The game
            // must not destroy its device while any layer still has submitted work.
            let _ = self.device.device_wait_idle();
            self.renderer.take();
            for v in self.buffers.drain(..) {
                self.device.destroy_framebuffer(v, None);
            }
            for v in self.views.drain(..) {
                self.device.destroy_image_view(v, None);
            }
            for v in self.semaphores.drain(..) {
                self.device.destroy_semaphore(v, None);
            }
            if self.pass != vk::RenderPass::null() {
                self.device.destroy_render_pass(self.pass, None);
            }
            for fence in self.fences.drain(..) {
                self.device.destroy_fence(fence, None);
            }
            if self.pool != vk::CommandPool::null() {
                self.device.destroy_command_pool(self.pool, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Run explicitly with Mesa lavapipe. Creates the real render pass, views,
    /// framebuffers, shaders and blend pipeline; no window or game is opened.
    #[test]
    #[ignore = "requires a Vulkan implementation; run with lavapipe explicitly"]
    fn sdr_pipelines_create_on_vulkan_for_eight_and_ten_bit_images() {
        unsafe {
            let loader = libc::dlopen(
                c"libvulkan.so.1".as_ptr(),
                libc::RTLD_NOW | libc::RTLD_LOCAL,
            );
            assert!(!loader.is_null());
            let gipa = libc::dlsym(loader, c"vkGetInstanceProcAddr".as_ptr());
            assert!(!gipa.is_null());
            let entry = ash::Entry::from_static_fn(ash::StaticFn {
                get_instance_proc_addr: std::mem::transmute(gipa),
            });
            let instance = entry
                .create_instance(&vk::InstanceCreateInfo::default(), None)
                .unwrap();
            let physical = instance.enumerate_physical_devices().unwrap()[0];
            let family = instance
                .get_physical_device_queue_family_properties(physical)
                .iter()
                .position(|q| q.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                .unwrap() as u32;
            let priorities = [1.];
            let queues = [vk::DeviceQueueCreateInfo::default()
                .queue_family_index(family)
                .queue_priorities(&priorities)];
            let device = instance
                .create_device(
                    physical,
                    &vk::DeviceCreateInfo::default().queue_create_infos(&queues),
                    None,
                )
                .unwrap();
            let memory = instance.get_physical_device_memory_properties(physical);
            for format in [
                vk::Format::B8G8R8A8_UNORM,
                vk::Format::R8G8B8A8_UNORM,
                vk::Format::B8G8R8A8_SRGB,
                vk::Format::R8G8B8A8_SRGB,
                vk::Format::A2B10G10R10_UNORM_PACK32,
                vk::Format::A2R10G10B10_UNORM_PACK32,
            ] {
                let extent = vk::Extent2D {
                    width: 32,
                    height: 32,
                };
                let image = device
                    .create_image(
                        &vk::ImageCreateInfo::default()
                            .image_type(vk::ImageType::TYPE_2D)
                            .format(format)
                            .extent(vk::Extent3D {
                                width: 32,
                                height: 32,
                                depth: 1,
                            })
                            .mip_levels(1)
                            .array_layers(1)
                            .samples(vk::SampleCountFlags::TYPE_1)
                            .tiling(vk::ImageTiling::OPTIMAL)
                            .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT),
                        None,
                    )
                    .unwrap();
                let requirements = device.get_image_memory_requirements(image);
                let memory_type = (0..memory.memory_type_count)
                    .find(|i| requirements.memory_type_bits & (1 << i) != 0)
                    .unwrap();
                let allocation = device
                    .allocate_memory(
                        &vk::MemoryAllocateInfo::default()
                            .allocation_size(requirements.size)
                            .memory_type_index(memory_type),
                        None,
                    )
                    .unwrap();
                device.bind_image_memory(image, allocation, 0).unwrap();
                let renderer = Render::new(
                    &instance,
                    physical,
                    device.clone(),
                    format,
                    extent,
                    &[image],
                    family,
                )
                .unwrap_or_else(|e| panic!("{format:?}: {e}"));
                drop(renderer);
                device.destroy_image(image, None);
                device.free_memory(allocation, None);
            }
            device.destroy_device(None);
            instance.destroy_instance(None);
            libc::dlclose(loader);
        }
    }
}
