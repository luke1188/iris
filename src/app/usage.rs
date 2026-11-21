//! CPU / GPU utilization for the stage debug overlay.

use std::time::{Duration, Instant};
use sysinfo::System;

const SAMPLE_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy)]
pub struct ResourceUsage {
    pub cpu_percent: f32,
    pub gpu_percent: Option<f32>,
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            cpu_percent: 0.0,
            gpu_percent: None,
        }
    }
}

pub struct UsageSampler {
    sys: System,
    last_sample: Instant,
    cached: ResourceUsage,
    gpu: GpuProbe,
}

impl UsageSampler {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        let mut gpu = GpuProbe::new();
        let gpu_percent = gpu.utilization();
        Self {
            sys,
            last_sample: Instant::now(),
            cached: ResourceUsage {
                cpu_percent: 0.0,
                gpu_percent,
            },
            gpu,
        }
    }

    pub fn snapshot(&self) -> ResourceUsage {
        self.cached
    }

    pub fn tick(&mut self) {
        if self.last_sample.elapsed() < SAMPLE_INTERVAL {
            return;
        }
        self.last_sample = Instant::now();
        self.sys.refresh_cpu_usage();
        self.cached.cpu_percent = self.sys.global_cpu_usage().clamp(0.0, 100.0);
        self.cached.gpu_percent = self.gpu.utilization();
    }
}

struct GpuProbe {
    #[cfg(target_os = "windows")]
    windows: Option<windows_gpu::Query>,
}

impl GpuProbe {
    fn new() -> Self {
        Self {
            #[cfg(target_os = "windows")]
            windows: windows_gpu::Query::new(),
        }
    }

    fn utilization(&mut self) -> Option<f32> {
        #[cfg(target_os = "macos")]
        {
            let _ = self;
            macos_gpu::utilization()
        }
        #[cfg(target_os = "windows")]
        {
            self.windows.as_mut().and_then(|q| q.utilization())
        }
        #[cfg(target_os = "linux")]
        {
            let _ = self;
            linux_gpu::utilization()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = self;
            None
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(non_camel_case_types)]
mod macos_gpu {
    use std::ffi::c_void;

    type CFTypeRef = *const c_void;
    type CFDictionaryRef = *const c_void;
    type CFMutableDictionaryRef = *mut c_void;
    type CFStringRef = *const c_void;
    type CFAllocatorRef = *const c_void;
    type CFTypeID = usize;
    type io_object_t = u32;
    type io_iterator_t = u32;
    type mach_port_t = u32;
    type kern_return_t = i32;

    const KERN_SUCCESS: kern_return_t = 0;
    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_CF_NUMBER_INT_TYPE: i32 = 9;
    const K_CF_NUMBER_DOUBLE_TYPE: i32 = 13;

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOServiceMatching(name: *const i8) -> CFMutableDictionaryRef;
        fn IOServiceGetMatchingServices(
            main_port: mach_port_t,
            matching: CFMutableDictionaryRef,
            existing: *mut io_iterator_t,
        ) -> kern_return_t;
        fn IOIteratorNext(iterator: io_iterator_t) -> io_object_t;
        fn IOObjectRelease(object: io_object_t) -> kern_return_t;
        fn IORegistryEntryCreateCFProperties(
            entry: io_object_t,
            properties: *mut CFMutableDictionaryRef,
            allocator: CFAllocatorRef,
            options: u32,
        ) -> kern_return_t;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: CFTypeRef);
        fn CFGetTypeID(cf: CFTypeRef) -> CFTypeID;
        fn CFDictionaryGetTypeID() -> CFTypeID;
        fn CFNumberGetTypeID() -> CFTypeID;
        fn CFDictionaryGetValue(the_dict: CFDictionaryRef, key: *const c_void) -> *const c_void;
        fn CFStringCreateWithCString(
            alloc: CFAllocatorRef,
            c_str: *const i8,
            encoding: u32,
        ) -> CFStringRef;
        fn CFNumberGetValue(number: *const c_void, the_type: i32, value_ptr: *mut c_void) -> u8;
    }

    pub fn utilization() -> Option<f32> {
        unsafe {
            let name = b"IOAccelerator\0";
            let matching = IOServiceMatching(name.as_ptr() as *const i8);
            if matching.is_null() {
                return None;
            }
            let mut iter: io_iterator_t = 0;
            // Port 0 = default master/main port. `matching` is consumed.
            if IOServiceGetMatchingServices(0, matching, &mut iter) != KERN_SUCCESS {
                return None;
            }
            let mut best: Option<f32> = None;
            loop {
                let entry = IOIteratorNext(iter);
                if entry == 0 {
                    break;
                }
                let mut props: CFMutableDictionaryRef = std::ptr::null_mut();
                if IORegistryEntryCreateCFProperties(entry, &mut props, std::ptr::null(), 0)
                    == KERN_SUCCESS
                    && !props.is_null()
                {
                    if let Some(v) = device_utilization(props as CFDictionaryRef) {
                        best = Some(best.map(|b| b.max(v)).unwrap_or(v));
                    }
                    CFRelease(props as CFTypeRef);
                }
                IOObjectRelease(entry);
            }
            IOObjectRelease(iter);
            best.map(|v| v.clamp(0.0, 100.0))
        }
    }

    unsafe fn device_utilization(props: CFDictionaryRef) -> Option<f32> {
        let stats = dict_get_dict(props, b"PerformanceStatistics\0")?;
        const KEYS: [&[u8]; 4] = [
            b"Device Utilization %\0",
            b"Device Utilization\0",
            b"GPU Core Utilization\0",
            b"Renderer Utilization %\0",
        ];
        for key in KEYS {
            if let Some(v) = dict_get_number(stats, key) {
                return Some(v);
            }
        }
        None
    }

    unsafe fn dict_get_dict(dict: CFDictionaryRef, key: &[u8]) -> Option<CFDictionaryRef> {
        let value = dict_get(dict, key)?;
        if CFGetTypeID(value) != CFDictionaryGetTypeID() {
            return None;
        }
        Some(value)
    }

    unsafe fn dict_get_number(dict: CFDictionaryRef, key: &[u8]) -> Option<f32> {
        let value = dict_get(dict, key)?;
        if CFGetTypeID(value) != CFNumberGetTypeID() {
            return None;
        }
        let mut d = 0.0f64;
        if CFNumberGetValue(value, K_CF_NUMBER_DOUBLE_TYPE, &mut d as *mut f64 as *mut c_void) != 0
            && d.is_finite()
        {
            return Some(d as f32);
        }
        let mut i = 0i32;
        if CFNumberGetValue(value, K_CF_NUMBER_INT_TYPE, &mut i as *mut i32 as *mut c_void) != 0 {
            return Some(i as f32);
        }
        None
    }

    unsafe fn dict_get(dict: CFDictionaryRef, key: &[u8]) -> Option<*const c_void> {
        let cf_key = CFStringCreateWithCString(
            std::ptr::null(),
            key.as_ptr() as *const i8,
            K_CF_STRING_ENCODING_UTF8,
        );
        if cf_key.is_null() {
            return None;
        }
        let value = CFDictionaryGetValue(dict, cf_key as *const c_void);
        CFRelease(cf_key as CFTypeRef);
        if value.is_null() {
            None
        } else {
            Some(value)
        }
    }
}

#[cfg(target_os = "windows")]
mod windows_gpu {
    use windows::core::w;
    use windows::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
        PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
    };

    pub struct Query {
        query: PDH_HQUERY,
        counter: PDH_HCOUNTER,
    }

    impl Query {
        pub fn new() -> Option<Self> {
            unsafe {
                let mut query = PDH_HQUERY::default();
                if PdhOpenQueryW(None, 0, &mut query) != 0 {
                    return None;
                }
                let mut counter = PDH_HCOUNTER::default();
                let status = PdhAddEnglishCounterW(
                    query,
                    w!("\\GPU Engine(*)\\Utilization Percentage"),
                    0,
                    &mut counter,
                );
                if status != 0 {
                    let _ = PdhCloseQuery(query);
                    return None;
                }
                let _ = PdhCollectQueryData(query);
                Some(Self { query, counter })
            }
        }

        pub fn utilization(&mut self) -> Option<f32> {
            unsafe {
                if PdhCollectQueryData(self.query) != 0 {
                    return None;
                }
                let mut buf_size: u32 = 0;
                let mut item_count: u32 = 0;
                let _ = PdhGetFormattedCounterArrayW(
                    self.counter,
                    PDH_FMT_DOUBLE,
                    &mut buf_size,
                    &mut item_count,
                    None,
                );
                if buf_size == 0 {
                    return None;
                }
                let mut buffer = vec![0u8; buf_size as usize];
                let items = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
                if PdhGetFormattedCounterArrayW(
                    self.counter,
                    PDH_FMT_DOUBLE,
                    &mut buf_size,
                    &mut item_count,
                    Some(items),
                ) != 0
                {
                    return None;
                }
                let slice = std::slice::from_raw_parts(items, item_count as usize);
                let mut sum_3d = 0.0f32;
                let mut max_any = 0.0f32;
                for item in slice {
                    let val = item.FmtValue.Anonymous.doubleValue as f32;
                    if !val.is_finite() {
                        continue;
                    }
                    max_any = max_any.max(val);
                    let name = item.szName.to_string().unwrap_or_default().to_ascii_lowercase();
                    if name.contains("engtype_3d") {
                        sum_3d += val;
                    }
                }
                let v = if sum_3d > 0.0 { sum_3d } else { max_any };
                Some(v.clamp(0.0, 100.0))
            }
        }
    }

    impl Drop for Query {
        fn drop(&mut self) {
            unsafe {
                let _ = PdhCloseQuery(self.query);
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod linux_gpu {
    const PATHS: &[&str] = &[
        "/sys/class/drm/card0/device/gpu_busy_percent",
        "/sys/class/drm/card1/device/gpu_busy_percent",
        "/sys/class/drm/card0/gt/gt0/busy_percentage",
    ];

    pub fn utilization() -> Option<f32> {
        for path in PATHS {
            if let Ok(s) = std::fs::read_to_string(path) {
                if let Ok(v) = s.trim().parse::<f32>() {
                    return Some(v.clamp(0.0, 100.0));
                }
            }
        }
        None
    }
}
