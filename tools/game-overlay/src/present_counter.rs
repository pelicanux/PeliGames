//! A passive Vulkan observer below AFMF or above MAKO. No input or GPU work.
//! Separate dispatch maps allow this layer and the renderer to share one DSO.
#![allow(non_snake_case)]
use ash::{vk, vk::Handle};
use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr},
    sync::{Mutex, OnceLock},
};
type Gipa = vk::PFN_vkGetInstanceProcAddr;
type Gdpa = vk::PFN_vkGetDeviceProcAddr;
#[repr(C)]
struct Negotiation {
    kind: u32,
    next: *mut c_void,
    version: u32,
    gipa: Option<Gipa>,
    gdpa: Option<Gdpa>,
    physical: Option<Gipa>,
}
#[repr(C)]
struct InstanceLink {
    next: *mut InstanceLink,
    gipa: Gipa,
    physical: Option<Gipa>,
}
#[repr(C)]
struct DeviceLink {
    next: *mut DeviceLink,
    gipa: Gipa,
    gdpa: Gdpa,
}
#[repr(C)]
struct Chain {
    kind: vk::StructureType,
    next: *const c_void,
    function: u32,
    link: *mut c_void,
}
#[derive(Clone, Copy)]
struct Instance {
    handle: vk::Instance,
    gipa: Gipa,
    physical: Option<Gipa>,
}
#[derive(Clone, Copy)]
struct Device {
    handle: vk::Device,
    gdpa: Gdpa,
}
static INSTANCES: OnceLock<Mutex<HashMap<usize, Instance>>> = OnceLock::new();
static DEVICES: OnceLock<Mutex<HashMap<usize, Device>>> = OnceLock::new();
unsafe fn key(handle: u64) -> usize {
    if handle == 0 {
        0
    } else {
        *(handle as *const usize)
    }
}
unsafe fn instance(handle: u64) -> Option<Instance> {
    INSTANCES
        .get_or_init(Default::default)
        .lock()
        .ok()?
        .get(&key(handle))
        .copied()
}
unsafe fn device(handle: u64) -> Option<Device> {
    DEVICES
        .get_or_init(Default::default)
        .lock()
        .ok()?
        .get(&key(handle))
        .copied()
}
unsafe fn function<T: Copy>(p: vk::PFN_vkVoidFunction) -> Option<T> {
    p.map(|p| std::mem::transmute_copy(&p))
}
macro_rules! address {
    ($f:ident) => {
        Some(std::mem::transmute($f as *const ()))
    };
}
unsafe fn chain(mut node: *const c_void, kind: vk::StructureType) -> *mut Chain {
    #[repr(C)]
    struct Header {
        kind: vk::StructureType,
        next: *const c_void,
    }
    for _ in 0..128 {
        if node.is_null() {
            break;
        }
        let header = &*(node as *const Header);
        if header.kind == kind && (*(node as *const Chain)).function == 0 {
            return node as *mut Chain;
        }
        node = header.next;
    }
    std::ptr::null_mut()
}
#[no_mangle]
unsafe extern "system" fn peliCounterNegotiateLoaderLayerInterfaceVersion(
    n: *mut Negotiation,
) -> vk::Result {
    if n.is_null() || (*n).kind != 1 || (*n).version < 1 {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    (*n).version = (*n).version.min(2);
    (*n).gipa = Some(gipa);
    (*n).gdpa = Some(gdpa);
    (*n).physical = if (*n).version == 2 {
        Some(physical)
    } else {
        None
    };
    vk::Result::SUCCESS
}
unsafe extern "system" fn physical(i: vk::Instance, name: *const c_char) -> vk::PFN_vkVoidFunction {
    if name.is_null() {
        return None;
    }
    if CStr::from_ptr(name).to_bytes() == b"vkCreateDevice" {
        return address!(create_device);
    }
    let data = instance(i.as_raw())?;
    if let Some(next) = data.physical {
        next(i, name)
    } else {
        (data.gipa)(i, name)
    }
}
unsafe extern "system" fn gipa(i: vk::Instance, name: *const c_char) -> vk::PFN_vkVoidFunction {
    if name.is_null() {
        return None;
    }
    match CStr::from_ptr(name).to_bytes() {
        b"vkGetInstanceProcAddr" => return address!(gipa),
        b"vkGetDeviceProcAddr" => return address!(gdpa),
        b"vkCreateInstance" => return address!(create_instance),
        _ => {}
    }
    let data = instance(i.as_raw())?;
    let next = (data.gipa)(i, name);
    next?;
    match CStr::from_ptr(name).to_bytes() {
        b"vkDestroyInstance" => address!(destroy_instance),
        b"vkCreateDevice" => address!(create_device),
        b"vkDestroyDevice" => address!(destroy_device),
        b"vkQueuePresentKHR" => address!(present),
        b"vkDestroySwapchainKHR" => address!(destroy_swapchain),
        _ => next,
    }
}
unsafe extern "system" fn gdpa(d: vk::Device, name: *const c_char) -> vk::PFN_vkVoidFunction {
    if name.is_null() {
        return None;
    }
    if CStr::from_ptr(name).to_bytes() == b"vkGetDeviceProcAddr" {
        return address!(gdpa);
    }
    let data = device(d.as_raw())?;
    let next = (data.gdpa)(d, name);
    next?;
    match CStr::from_ptr(name).to_bytes() {
        b"vkDestroyDevice" => address!(destroy_device),
        b"vkQueuePresentKHR" => address!(present),
        b"vkDestroySwapchainKHR" => address!(destroy_swapchain),
        _ => next,
    }
}
unsafe extern "system" fn create_instance(
    info: *const vk::InstanceCreateInfo<'_>,
    a: *const vk::AllocationCallbacks<'_>,
    out: *mut vk::Instance,
) -> vk::Result {
    if info.is_null() || out.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    let c = chain(
        (*info).p_next,
        vk::StructureType::LOADER_INSTANCE_CREATE_INFO,
    );
    if c.is_null() || (*c).link.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    let link = &*((*c).link as *const InstanceLink);
    let next = link.gipa;
    let physical = link.physical;
    let Some(create) = function::<vk::PFN_vkCreateInstance>(next(
        vk::Instance::null(),
        c"vkCreateInstance".as_ptr(),
    )) else {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    (*c).link = link.next.cast();
    let result = create(info, a, out);
    if result == vk::Result::SUCCESS {
        if let Ok(mut map) = INSTANCES.get_or_init(Default::default).lock() {
            map.insert(
                key((*out).as_raw()),
                Instance {
                    handle: *out,
                    gipa: next,
                    physical,
                },
            );
        }
    }
    result
}
unsafe extern "system" fn destroy_instance(i: vk::Instance, a: *const vk::AllocationCallbacks<'_>) {
    if let Some(data) = instance(i.as_raw()) {
        if let Ok(mut map) = INSTANCES.get_or_init(Default::default).lock() {
            map.remove(&key(i.as_raw()));
        }
        if let Some(next) =
            function::<vk::PFN_vkDestroyInstance>((data.gipa)(i, c"vkDestroyInstance".as_ptr()))
        {
            next(i, a);
        }
    }
}
unsafe extern "system" fn create_device(
    p: vk::PhysicalDevice,
    info: *const vk::DeviceCreateInfo<'_>,
    a: *const vk::AllocationCallbacks<'_>,
    out: *mut vk::Device,
) -> vk::Result {
    if info.is_null() || out.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    let Some(inst) = instance(p.as_raw()) else {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    let c = chain((*info).p_next, vk::StructureType::LOADER_DEVICE_CREATE_INFO);
    if c.is_null() || (*c).link.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    let link = &*((*c).link as *const DeviceLink);
    let next = link.gdpa;
    let Some(create) =
        function::<vk::PFN_vkCreateDevice>((link.gipa)(inst.handle, c"vkCreateDevice".as_ptr()))
    else {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    (*c).link = link.next.cast();
    let result = create(p, info, a, out);
    if result == vk::Result::SUCCESS {
        if let Ok(mut map) = DEVICES.get_or_init(Default::default).lock() {
            map.insert(
                key((*out).as_raw()),
                Device {
                    handle: *out,
                    gdpa: next,
                },
            );
        }
    }
    result
}
unsafe extern "system" fn destroy_device(d: vk::Device, a: *const vk::AllocationCallbacks<'_>) {
    if let Some(data) = device(d.as_raw()) {
        if let Ok(mut map) = DEVICES.get_or_init(Default::default).lock() {
            map.remove(&key(d.as_raw()));
        }
        if let Some(next) =
            function::<vk::PFN_vkDestroyDevice>((data.gdpa)(d, c"vkDestroyDevice".as_ptr()))
        {
            next(d, a);
        }
    }
}
unsafe extern "system" fn destroy_swapchain(
    d: vk::Device,
    s: vk::SwapchainKHR,
    a: *const vk::AllocationCallbacks<'_>,
) {
    if let Some(data) = device(d.as_raw()) {
        if let Some(next) = function::<vk::PFN_vkDestroySwapchainKHR>((data.gdpa)(
            d,
            c"vkDestroySwapchainKHR".as_ptr(),
        )) {
            next(d, s, a);
        }
        crate::present_metrics::forget(s.as_raw());
    }
}
unsafe extern "system" fn present(q: vk::Queue, info: *const vk::PresentInfoKHR<'_>) -> vk::Result {
    let Some(data) = device(q.as_raw()) else {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    let Some(next) = function::<vk::PFN_vkQueuePresentKHR>((data.gdpa)(
        data.handle,
        c"vkQueuePresentKHR".as_ptr(),
    )) else {
        return vk::Result::ERROR_EXTENSION_NOT_PRESENT;
    };
    // Pass the original metadata, semaphores and result array through untouched.
    let result = next(q, info);
    observe(
        info,
        result,
        crate::present_metrics::generator().is_some_and(|generator| generator.counter_is_source()),
        crate::layer::primary_swapchain,
    );
    result
}
unsafe fn observe(
    info: *const vk::PresentInfoKHR<'_>,
    result: vk::Result,
    source: bool,
    primary: impl Fn(u64) -> bool,
) {
    if !info.is_null() && !(*info).p_swapchains.is_null() {
        for index in 0..(*info).swapchain_count as usize {
            let success = if (*info).p_results.is_null() {
                result.as_raw() >= 0
            } else {
                (*(*info).p_results.add(index)).as_raw() >= 0
            };
            let id = (*(*info).p_swapchains.add(index)).as_raw();
            if success && (!source || primary(id)) {
                crate::present_metrics::record(id, source);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negotiation_is_separate_from_the_renderer_and_rejects_invalid_versions() {
        unsafe {
            assert_eq!(
                peliCounterNegotiateLoaderLayerInterfaceVersion(std::ptr::null_mut()),
                vk::Result::ERROR_INITIALIZATION_FAILED
            );
            let mut n = Negotiation {
                kind: 1,
                next: std::ptr::null_mut(),
                version: 2,
                gipa: None,
                gdpa: None,
                physical: None,
            };
            assert_eq!(
                peliCounterNegotiateLoaderLayerInterfaceVersion(&mut n),
                vk::Result::SUCCESS
            );
            assert!(n.physical.is_some());
            assert_ne!(
                n.gipa.unwrap() as usize,
                crate::layer::vkGetInstanceProcAddr as *const () as usize
            );
            n.version = 0;
            assert_eq!(
                peliCounterNegotiateLoaderLayerInterfaceVersion(&mut n),
                vk::Result::ERROR_INITIALIZATION_FAILED
            );
        }
    }
    #[test]
    fn loader_links_create_and_destroy_separate_instance_and_device_dispatch() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static INSTANCE: AtomicU64 = AtomicU64::new(0);
        static DEVICE: AtomicU64 = AtomicU64::new(0);
        unsafe extern "system" fn create_i(
            info: *const vk::InstanceCreateInfo<'_>,
            _a: *const vk::AllocationCallbacks<'_>,
            out: *mut vk::Instance,
        ) -> vk::Result {
            assert!((*((*info).p_next as *const Chain)).link.is_null());
            *out = vk::Instance::from_raw(INSTANCE.load(Ordering::Relaxed));
            vk::Result::SUCCESS
        }
        unsafe extern "system" fn create_d(
            _p: vk::PhysicalDevice,
            info: *const vk::DeviceCreateInfo<'_>,
            _a: *const vk::AllocationCallbacks<'_>,
            out: *mut vk::Device,
        ) -> vk::Result {
            assert!((*((*info).p_next as *const Chain)).link.is_null());
            *out = vk::Device::from_raw(DEVICE.load(Ordering::Relaxed));
            vk::Result::SUCCESS
        }
        unsafe extern "system" fn destroy_i(
            _i: vk::Instance,
            _a: *const vk::AllocationCallbacks<'_>,
        ) {
        }
        unsafe extern "system" fn destroy_d(
            _d: vk::Device,
            _a: *const vk::AllocationCallbacks<'_>,
        ) {
        }
        unsafe extern "system" fn next_i(
            _i: vk::Instance,
            n: *const c_char,
        ) -> vk::PFN_vkVoidFunction {
            match CStr::from_ptr(n).to_bytes() {
                b"vkCreateInstance" => Some(std::mem::transmute(create_i as *const ())),
                b"vkCreateDevice" => Some(std::mem::transmute(create_d as *const ())),
                b"vkDestroyInstance" => Some(std::mem::transmute(destroy_i as *const ())),
                _ => None,
            }
        }
        unsafe extern "system" fn next_d(
            _d: vk::Device,
            n: *const c_char,
        ) -> vk::PFN_vkVoidFunction {
            if CStr::from_ptr(n).to_bytes() == b"vkDestroyDevice" {
                Some(std::mem::transmute(destroy_d as *const ()))
            } else {
                None
            }
        }
        let instance_dispatch = Box::new(0x70656c69usize);
        let device_dispatch = Box::new(0x70656c6ausize);
        let iraw = (&*instance_dispatch as *const usize) as u64;
        let draw = (&*device_dispatch as *const usize) as u64;
        INSTANCE.store(iraw, Ordering::Relaxed);
        DEVICE.store(draw, Ordering::Relaxed);
        unsafe {
            let mut link = InstanceLink {
                next: std::ptr::null_mut(),
                gipa: next_i,
                physical: None,
            };
            let c = Chain {
                kind: vk::StructureType::LOADER_INSTANCE_CREATE_INFO,
                next: std::ptr::null(),
                function: 0,
                link: (&mut link as *mut InstanceLink).cast(),
            };
            let mut info = vk::InstanceCreateInfo::default();
            info.p_next = (&c as *const Chain).cast();
            let mut inst = vk::Instance::null();
            assert_eq!(
                create_instance(&info, std::ptr::null(), &mut inst),
                vk::Result::SUCCESS
            );
            assert!(gipa(inst, c"vkCreateDevice".as_ptr()).is_some());
            let mut link = DeviceLink {
                next: std::ptr::null_mut(),
                gipa: next_i,
                gdpa: next_d,
            };
            let c = Chain {
                kind: vk::StructureType::LOADER_DEVICE_CREATE_INFO,
                next: std::ptr::null(),
                function: 0,
                link: (&mut link as *mut DeviceLink).cast(),
            };
            let mut info = vk::DeviceCreateInfo::default();
            info.p_next = (&c as *const Chain).cast();
            let mut dev = vk::Device::null();
            assert_eq!(
                create_device(
                    vk::PhysicalDevice::from_raw(iraw),
                    &info,
                    std::ptr::null(),
                    &mut dev
                ),
                vk::Result::SUCCESS
            );
            assert!(gdpa(dev, c"vkDestroyDevice".as_ptr()).is_some());
            assert!(gdpa(dev, c"vkUnknownExtension".as_ptr()).is_none());
            destroy_device(dev, std::ptr::null());
            assert!(device(draw).is_none());
            destroy_instance(inst, std::ptr::null());
            assert!(instance(iraw).is_none());
        }
    }

    #[test]
    fn native_presenter_counts_actual_successes_and_forwards_metadata_untouched() {
        use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
        static EXPECTED: AtomicUsize = AtomicUsize::new(0);
        static RESULT: AtomicI32 = AtomicI32::new(0);
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "system" fn driver(
            _q: vk::Queue,
            info: *const vk::PresentInfoKHR<'_>,
        ) -> vk::Result {
            assert_eq!(info as usize, EXPECTED.load(Ordering::Relaxed));
            assert!(!(*info).p_next.is_null());
            CALLS.fetch_add(1, Ordering::Relaxed);
            let result = vk::Result::from_raw(RESULT.load(Ordering::Relaxed));
            if !(*info).p_results.is_null() {
                *(*info).p_results = result;
            }
            result
        }
        unsafe extern "system" fn next(
            _d: vk::Device,
            name: *const c_char,
        ) -> vk::PFN_vkVoidFunction {
            if CStr::from_ptr(name).to_bytes() == b"vkQueuePresentKHR" {
                Some(std::mem::transmute(driver as *const ()))
            } else {
                None
            }
        }
        // A fake dispatchable handle, not a real Vulkan device or user setting.
        let dispatch = Box::new(0x70656c696370usize);
        let raw = (&*dispatch as *const usize) as u64;
        let id = 0x70656c696370u64;
        let swapchains = [vk::SwapchainKHR::from_raw(id)];
        let indices = [0];
        let ids = [42];
        let mut results = [vk::Result::SUCCESS];
        let mut metadata = vk::PresentIdKHR::default().present_ids(&ids);
        let info = vk::PresentInfoKHR::default()
            .swapchains(&swapchains)
            .image_indices(&indices)
            .results(&mut results)
            .push_next(&mut metadata);
        let info_address = &info as *const _ as usize;
        EXPECTED.store(info_address, Ordering::Relaxed);
        unsafe {
            DEVICES
                .get_or_init(Default::default)
                .lock()
                .unwrap()
                .insert(
                    key(raw),
                    Device {
                        handle: vk::Device::from_raw(raw),
                        gdpa: next,
                    },
                );
            // One real frame, two actual submissions made on a native presenter.
            std::thread::spawn(move || {
                let info = info_address as *const vk::PresentInfoKHR<'_>;
                assert_eq!(present(vk::Queue::from_raw(raw), info), vk::Result::SUCCESS);
                assert_eq!(present(vk::Queue::from_raw(raw), info), vk::Result::SUCCESS);
            })
            .join()
            .unwrap();
            crate::present_metrics::record(id, true);
            assert_eq!(crate::present_metrics::pending_counts(id), Some((1, 2)));
            RESULT.store(
                vk::Result::ERROR_OUT_OF_DATE_KHR.as_raw(),
                Ordering::Relaxed,
            );
            assert_eq!(
                present(vk::Queue::from_raw(raw), &info),
                vk::Result::ERROR_OUT_OF_DATE_KHR
            );
            assert_eq!(crate::present_metrics::pending_counts(id), Some((1, 2)));
            RESULT.store(vk::Result::SUBOPTIMAL_KHR.as_raw(), Ordering::Relaxed);
            assert_eq!(
                present(vk::Queue::from_raw(raw), &info),
                vk::Result::SUBOPTIMAL_KHR
            );
            assert_eq!(crate::present_metrics::pending_counts(id), Some((1, 3)));
            assert_eq!(CALLS.load(Ordering::Relaxed), 4);
            assert_eq!((*info.p_swapchains).as_raw(), id);
            assert_eq!((*info.p_image_indices), 0);
            assert_eq!((*info.p_results), vk::Result::SUBOPTIMAL_KHR);
            DEVICES.get().unwrap().lock().unwrap().remove(&key(raw));
        }
        crate::present_metrics::forget(id);
        assert!(crate::present_metrics::pending_counts(id).is_none());
    }

    #[test]
    fn mako_observer_counts_only_original_presents_of_the_renderer_swapchain() {
        let id = 0x70656c696d616b6fu64;
        let other = id + 1;
        let swapchains = [
            vk::SwapchainKHR::from_raw(id),
            vk::SwapchainKHR::from_raw(other),
        ];
        let indices = [0, 0];
        let mut results = [vk::Result::SUBOPTIMAL_KHR, vk::Result::SUCCESS];
        let info = vk::PresentInfoKHR::default()
            .swapchains(&swapchains)
            .image_indices(&indices)
            .results(&mut results);
        unsafe {
            // An original game call reaches MAKO once; two accepted outputs
            // already reached our renderer while MAKO handled this invocation.
            crate::present_metrics::record(id, false);
            crate::present_metrics::record(id, false);
            observe(&info, vk::Result::SUCCESS, true, |value| value == id);
            assert_eq!(crate::present_metrics::pending_counts(id), Some((1, 2)));
            assert!(crate::present_metrics::pending_counts(other).is_none());
            *info.p_results = vk::Result::ERROR_OUT_OF_DATE_KHR;
            observe(&info, vk::Result::ERROR_OUT_OF_DATE_KHR, true, |value| {
                value == id
            });
            assert_eq!(crate::present_metrics::pending_counts(id), Some((1, 2)));
            let failed = vk::PresentInfoKHR::default()
                .swapchains(&swapchains)
                .image_indices(&indices);
            observe(&failed, vk::Result::ERROR_DEVICE_LOST, true, |value| {
                value == id
            });
            assert_eq!(crate::present_metrics::pending_counts(id), Some((1, 2)));
            assert_eq!((*info.p_swapchains).as_raw(), id);
            assert_eq!((*info.p_results), vk::Result::ERROR_OUT_OF_DATE_KHR);
        }
        crate::present_metrics::forget(id);
        crate::present_metrics::forget(other);
    }

    #[test]
    fn unknown_extension_headers_are_skipped_without_reading_loader_fields() {
        unsafe {
            #[repr(C)]
            struct Header {
                kind: vk::StructureType,
                next: *const c_void,
            }
            let c = Chain {
                kind: vk::StructureType::LOADER_DEVICE_CREATE_INFO,
                next: std::ptr::null(),
                function: 0,
                link: std::ptr::null_mut(),
            };
            let h = Header {
                kind: vk::StructureType::PRESENT_ID_KHR,
                next: (&c as *const Chain).cast(),
            };
            assert_eq!(
                chain(
                    (&h as *const Header).cast(),
                    vk::StructureType::LOADER_DEVICE_CREATE_INFO
                ),
                &c as *const Chain as *mut Chain
            );
        }
    }
}
