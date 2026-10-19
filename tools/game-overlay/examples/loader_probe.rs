//! Isolated loader/RTLD_LOCAL probe. Run through the production overlay runner
//! with Mesa lavapipe; it opens no display and never reads real game profiles.
use ash::{vk, Entry};
use std::{
    ffi::{CStr, CString},
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
#[path = "../symbols.rs"]
mod abi;

unsafe fn open(path: &CStr, flags: i32) -> *mut libc::c_void {
    let handle = libc::dlopen(path.as_ptr(), flags);
    assert!(!handle.is_null(), "dlopen: {}", path.to_string_lossy());
    handle
}
unsafe fn audit() -> usize {
    let directory =
        PathBuf::from(std::env::var_os("PELI_PROBE_OVERLAY_DIR").expect("probe directory"));
    let path = CString::new(
        directory
            .join("libpeligames_game_overlay.so")
            .as_os_str()
            .as_encoded_bytes(),
    )
    .unwrap();
    let renderer = open(&path, libc::RTLD_NOW | libc::RTLD_NOLOAD);
    let table = libc::dlsym(renderer, c"peli_overlay_originals".as_ptr()) as *const AtomicUsize;
    assert!(!table.is_null(), "shared original-symbol table");
    let mut count = 0;
    for (library, owner) in [
        (c"libwayland-client.so.0", abi::WAYLAND),
        (c"libX11.so.6", abi::X11),
    ] {
        let handle = open(library, libc::RTLD_NOW | libc::RTLD_LOCAL);
        for (i, (name, defining, redirect)) in abi::SYMBOLS.iter().enumerate() {
            if *defining != owner {
                continue;
            }
            let symbol = CString::new(*name).unwrap();
            let requested = libc::dlsym(handle, symbol.as_ptr()) as usize;
            let original = (*table.add(i)).load(Ordering::Acquire);
            assert_ne!(original, 0, "original {}", symbol.to_string_lossy());
            if *redirect {
                let wrapper = libc::dlsym(renderer, symbol.as_ptr()) as usize;
                assert_ne!(wrapper, 0, "wrapper {}", symbol.to_string_lossy());
                assert_ne!(wrapper, original, "interposed {}", symbol.to_string_lossy());
                assert_eq!(
                    requested,
                    wrapper,
                    "RTLD_LOCAL redirection {}",
                    symbol.to_string_lossy()
                );
            } else {
                assert_eq!(requested, original, "forward {}", symbol.to_string_lossy());
            }
            count += 1;
        }
        assert_eq!(libc::dlclose(handle), 0);
        // The renderer stores original addresses, not a dlopen reference. All
        // slots belonging to this unloaded DSO must be invalidated together.
        for (i, (_, defining, _)) in abi::SYMBOLS.iter().enumerate() {
            if *defining == owner {
                assert_eq!(
                    (*table.add(i)).load(Ordering::Acquire),
                    0,
                    "dlclose original slot {i}"
                );
            }
        }
    }
    if let Some(library) = std::env::var_os("PELI_PROBE_WINELAND_LIBRARY") {
        let library = CString::new(library.as_encoded_bytes()).unwrap();
        let handle = open(&library, libc::RTLD_NOW | libc::RTLD_LOCAL);
        let index = abi::SYMBOLS
            .iter()
            .position(|(_, owner, _)| *owner == abi::WINELAND)
            .unwrap();
        let symbol = c"__wine_wayland_vulkan_register_surface_v1";
        let requested = libc::dlsym(handle, symbol.as_ptr());
        let original = (*table.add(index)).load(Ordering::Acquire);
        let wrapper = libc::dlsym(renderer, symbol.as_ptr());
        assert_ne!(original, 0, "original Wineland v1");
        assert!(!wrapper.is_null(), "Wineland v1 wrapper");
        assert_eq!(requested, wrapper, "RTLD_LOCAL Wineland v1 redirection");
        assert_ne!(requested as usize, original);
        // A real translator with no registered Vulkan instance must return
        // its own failure through our wrapper, without opening a display.
        type Register = unsafe extern "C" fn(
            *mut libc::c_void,
            *mut libc::c_void,
            u64,
            *mut libc::c_void,
            *mut libc::c_void,
        ) -> i32;
        let register: Register = std::mem::transmute(requested);
        assert_eq!(
            register(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            -3
        );
        assert_eq!(libc::dlclose(handle), 0);
        assert_eq!(
            (*table.add(index)).load(Ordering::Acquire),
            0,
            "Wineland unload invalidation"
        );
        count += 1;
    }
    libc::dlclose(renderer);
    count
}
unsafe fn vulkan() -> String {
    let loader = open(c"libvulkan.so.1", libc::RTLD_NOW | libc::RTLD_LOCAL);
    let proc = libc::dlsym(loader, c"vkGetInstanceProcAddr".as_ptr());
    assert!(!proc.is_null());
    let entry = Entry::from_static_fn(ash::StaticFn {
        get_instance_proc_addr: std::mem::transmute(proc),
    });
    let app = vk::ApplicationInfo::default()
        .application_name(c"PeliGames loader probe")
        .api_version(vk::API_VERSION_1_1);
    let instance = entry
        .create_instance(
            &vk::InstanceCreateInfo::default().application_info(&app),
            None,
        )
        .expect("vkCreateInstance");
    let devices = instance
        .enumerate_physical_devices()
        .expect("physical devices");
    let physical = *devices.first().expect("software Vulkan device");
    let properties = instance.get_physical_device_properties(physical);
    let mut properties2 = vk::PhysicalDeviceProperties2::default();
    instance.get_physical_device_properties2(physical, &mut properties2);
    assert_eq!(properties.device_id, properties2.properties.device_id);
    let family = instance
        .get_physical_device_queue_family_properties(physical)
        .iter()
        .position(|p| p.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        .expect("graphics queue") as u32;
    let priorities = [1.0_f32];
    let queues = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(family)
        .queue_priorities(&priorities)];
    let device = instance
        .create_device(
            physical,
            &vk::DeviceCreateInfo::default().queue_create_infos(&queues),
            None,
        )
        .expect("vkCreateDevice");
    assert_ne!(device.get_device_queue(family, 0), vk::Queue::null());
    device.device_wait_idle().expect("idle");
    let name = CStr::from_ptr(properties.device_name.as_ptr())
        .to_string_lossy()
        .into_owned();
    device.destroy_device(None);
    instance.destroy_instance(None);
    libc::dlclose(loader);
    name
}
fn main() {
    unsafe {
        let symbols = audit();
        let device = vulkan();
        println!(
            "{}",
            serde_json::json!({"audit_symbols":symbols,"rtld_local":true,"dlclose_invalidates":true,"vulkan_instance_device":true,"device":device,"graphical_test":false})
        );
    }
}
