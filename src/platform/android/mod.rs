use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::ptr;
use std::sync::OnceLock;

use crate::models::metrics::MetricValue;
use crate::models::telemetry::{GpuTelemetry, GpuVendor, StorageDeviceTelemetry};

const PROP_VALUE_MAX: usize = 92;

extern "C" {
    fn __system_property_get(name: *const libc::c_char, value: *mut libc::c_char) -> libc::c_int;
}

pub fn get_system_property(name: &str) -> Option<String> {
    let c_name = CString::new(name).ok()?;
    let mut buf = [0_u8; PROP_VALUE_MAX];
    let len = unsafe { __system_property_get(c_name.as_ptr(), buf.as_mut_ptr().cast()) };
    if len <= 0 {
        return None;
    }
    let slice = &buf[..(len as usize).min(PROP_VALUE_MAX - 1)];
    let val = String::from_utf8_lossy(slice).trim().to_string();
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

pub fn android_os_name() -> MetricValue<String> {
    let release = get_system_property("ro.build.version.release");
    let sdk = get_system_property("ro.build.version.sdk");
    match (release, sdk) {
        (Some(rel), Some(api)) => MetricValue::Available(format!("Android {rel} (API {api})")),
        (Some(rel), None) => MetricValue::Available(format!("Android {rel}")),
        (None, Some(api)) => MetricValue::Available(format!("Android (API {api})")),
        (None, None) => MetricValue::Available("Android".to_string()),
    }
}

pub fn android_device_name() -> MetricValue<String> {
    let mfg = get_system_property("ro.product.manufacturer");
    let model = get_system_property("ro.product.model");
    match (mfg, model) {
        (Some(m), Some(mdl)) => {
            if mdl
                .to_ascii_lowercase()
                .starts_with(&m.to_ascii_lowercase())
            {
                MetricValue::Available(mdl)
            } else {
                MetricValue::Available(format!("{m} {mdl}"))
            }
        }
        (None, Some(mdl)) => MetricValue::Available(mdl),
        (Some(m), None) => MetricValue::Available(m),
        (None, None) => MetricValue::Unavailable("Device model unavailable".to_string()),
    }
}

pub fn android_kernel_version() -> MetricValue<String> {
    let mut uts: libc::utsname = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::uname(&mut uts) };
    if rc != 0 {
        return MetricValue::Unavailable("uname failed".to_string());
    }
    let rel = unsafe { CStr::from_ptr(uts.release.as_ptr()) };
    let s = rel.to_string_lossy().trim().to_string();
    if s.is_empty() {
        MetricValue::Unavailable("Empty kernel release".to_string())
    } else {
        MetricValue::Available(s)
    }
}

pub fn android_uptime_seconds() -> MetricValue<u64> {
    let mut info: libc::sysinfo = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::sysinfo(&mut info) };
    if rc == 0 && info.uptime >= 0 {
        MetricValue::Available(info.uptime as u64)
    } else {
        MetricValue::Unavailable("sysinfo uptime unavailable".to_string())
    }
}

pub fn android_soc_model() -> Option<String> {
    get_system_property("ro.soc.model")
        .or_else(|| get_system_property("ro.hardware.chipname"))
        .or_else(|| get_system_property("ro.board.platform"))
        .or_else(|| get_system_property("ro.hardware"))
}

pub fn android_soc_vendor() -> Option<String> {
    let raw = get_system_property("ro.soc.manufacturer")
        .or_else(|| get_system_property("ro.hardware.chipname"))?;
    let normalized = match raw.trim().to_ascii_lowercase().as_str() {
        "qti" | "qualcomm" | "qcom" => "Qualcomm".to_string(),
        "mediatek" | "mtk" => "MediaTek".to_string(),
        "samsung" | "exynos" | "slsi" => "Samsung".to_string(),
        "google" => "Google".to_string(),
        "hisilicon" => "HiSilicon".to_string(),
        _ => raw.trim().to_string(),
    };
    Some(normalized)
}

pub fn android_cpu_temperature() -> (MetricValue<f64>, Option<String>) {
    read_android_thermal_zone(&["cpu", "soc", "tsens", "ap_ntc"])
}

fn read_android_thermal_zone(keywords: &[&str]) -> (MetricValue<f64>, Option<String>) {
    let thermal_root = Path::new("/sys/class/thermal");
    let entries = match fs::read_dir(thermal_root) {
        Ok(e) => e,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            return (
                MetricValue::PermissionError(
                    "Thermal sysfs restricted on unprivileged Android".to_string(),
                ),
                None,
            );
        }
        Err(_) => {
            return (
                MetricValue::Unsupported("Thermal zones unavailable".to_string()),
                None,
            );
        }
    };

    let mut saw_permission_denied = false;
    let mut saw_read_failure = false;
    for entry in entries.flatten() {
        let dir = entry.path();
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("thermal_zone") {
            continue;
        }
        let zone_type = match fs::read_to_string(dir.join("type")) {
            Ok(value) => value,
            Err(err) => {
                saw_permission_denied |= err.kind() == ErrorKind::PermissionDenied;
                continue;
            }
        };
        let zt = zone_type.trim();
        let lower = zt.to_ascii_lowercase();
        if !keywords.iter().any(|kw| lower.contains(kw)) {
            continue;
        }
        if keywords.iter().any(|k| *k == "gpu" || *k == "gpuss")
            && !crate::platform::gpu_probe::gpu_thermal_type(&lower)
        {
            continue;
        }

        match fs::read_to_string(dir.join("temp")) {
            Ok(raw) => {
                if let Ok(val) = raw.trim().parse::<i64>() {
                    // Android thermal zones are typically millidegrees C, occasionally degrees C
                    let celsius = if val > 200 {
                        val as f64 / 1000.0
                    } else {
                        val as f64
                    };
                    if (0.0..=140.0).contains(&celsius) {
                        return (MetricValue::Available(celsius), Some(zt.to_string()));
                    }
                }
                saw_read_failure = true;
            }
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                saw_permission_denied = true;
            }
            Err(_) => {
                saw_read_failure = true;
            }
        }
    }

    if saw_read_failure {
        (
            MetricValue::Unavailable(
                "Sensor read failure: GPU/thermal zone returned invalid data or I/O failure".into(),
            ),
            None,
        )
    } else if saw_permission_denied {
        (
            MetricValue::PermissionError(
                "Permission denied reading thermal sysfs; Android sandbox or SELinux may restrict access".to_string(),
            ),
            None,
        )
    } else {
        (
            MetricValue::Unsupported("No readable thermal zone".to_string()),
            None,
        )
    }
}

#[derive(Debug, Clone)]
struct AndroidGpuIdentity {
    name: String,
    vendor: GpuVendor,
    vendor_name: String,
    driver_version: MetricValue<String>,
}

fn cached_android_gpu_identity() -> &'static AndroidGpuIdentity {
    static GPU_IDENTITY: OnceLock<AndroidGpuIdentity> = OnceLock::new();
    GPU_IDENTITY.get_or_init(detect_android_gpu_identity)
}

fn detect_android_gpu_identity() -> AndroidGpuIdentity {
    let egl_prop = get_system_property("ro.hardware.egl");
    let vk_prop = get_system_property("ro.hardware.vulkan");
    let board_prop = get_system_property("ro.board.platform");

    if let Some(vk) = probe_vulkan_gpu() {
        let driver = match (vk_prop.as_deref(), egl_prop.as_deref()) {
            (Some(vp), _) => format!("Vulkan {} ({vp})", vk.api_version_str),
            (None, Some(ep)) => format!("Vulkan {} ({ep})", vk.api_version_str),
            (None, None) => format!("Vulkan {}", vk.api_version_str),
        };
        return AndroidGpuIdentity {
            name: vk.device_name,
            vendor: vk.vendor,
            vendor_name: vk.vendor_name,
            driver_version: MetricValue::Available(driver),
        };
    }

    if let Some(egl) = probe_egl_gpu() {
        let (vendor, vendor_name) = classify_gpu_vendor(&egl.vendor, &egl.renderer);
        let driver = if !egl.version.is_empty() {
            MetricValue::Available(egl.version)
        } else if let Some(ep) = egl_prop {
            MetricValue::Available(format!("EGL: {ep}"))
        } else {
            MetricValue::Unavailable("GPU driver version unavailable".to_string())
        };
        return AndroidGpuIdentity {
            name: egl.renderer,
            vendor,
            vendor_name,
            driver_version: driver,
        };
    }

    let hint = egl_prop
        .as_deref()
        .or(vk_prop.as_deref())
        .or(board_prop.as_deref())
        .unwrap_or("Mobile GPU");
    let (vendor, vendor_name) = classify_gpu_vendor("", hint);
    let display_name = match vendor_name.as_str() {
        "Qualcomm" => format!("Qualcomm Adreno ({hint})"),
        "ARM" => format!("ARM Mali ({hint})"),
        "Imagination" => format!("PowerVR ({hint})"),
        _ => format!("Integrated GPU ({hint})"),
    };

    let driver = match (egl_prop, vk_prop) {
        (Some(e), Some(v)) if e != v => MetricValue::Available(format!("EGL: {e} / Vulkan: {v}")),
        (Some(e), _) => MetricValue::Available(format!("EGL: {e}")),
        (_, Some(v)) => MetricValue::Available(format!("Vulkan: {v}")),
        _ => MetricValue::Unavailable("GPU driver property unavailable".to_string()),
    };

    AndroidGpuIdentity {
        name: display_name,
        vendor,
        vendor_name,
        driver_version: driver,
    }
}

fn classify_gpu_vendor(vendor_hint: &str, renderer_hint: &str) -> (GpuVendor, String) {
    let combined = format!("{vendor_hint} {renderer_hint}").to_ascii_lowercase();
    if combined.contains("adreno") || combined.contains("qualcomm") || combined.contains("qcom") {
        (GpuVendor::Other, "Qualcomm".to_string())
    } else if combined.contains("mali")
        || combined.contains("immortalis")
        || combined.contains("arm")
    {
        (GpuVendor::Other, "ARM".to_string())
    } else if combined.contains("powervr") || combined.contains("imagination") {
        (GpuVendor::Other, "Imagination".to_string())
    } else if combined.contains("nvidia") || combined.contains("tegra") {
        (GpuVendor::Nvidia, "NVIDIA".to_string())
    } else if combined.contains("amd")
        || combined.contains("radeon")
        || combined.contains("xclipse")
    {
        (GpuVendor::Amd, "AMD".to_string())
    } else if combined.contains("intel") {
        (GpuVendor::Intel, "Intel".to_string())
    } else {
        (GpuVendor::Other, "Android".to_string())
    }
}

struct VulkanProbeResult {
    device_name: String,
    vendor: GpuVendor,
    vendor_name: String,
    api_version_str: String,
}

#[repr(C)]
struct VkApplicationInfo {
    s_type: u32,
    p_next: *const libc::c_void,
    p_application_name: *const libc::c_char,
    application_version: u32,
    p_engine_name: *const libc::c_char,
    engine_version: u32,
    api_version: u32,
}

#[repr(C)]
struct VkInstanceCreateInfo {
    s_type: u32,
    p_next: *const libc::c_void,
    flags: u32,
    p_application_info: *const VkApplicationInfo,
    enabled_layer_count: u32,
    pp_enabled_layer_names: *const *const libc::c_char,
    enabled_extension_count: u32,
    pp_enabled_extension_names: *const *const libc::c_char,
}

#[repr(C)]
struct VkPhysicalDevicePropertiesRaw {
    api_version: u32,
    driver_version: u32,
    vendor_id: u32,
    device_id: u32,
    device_type: u32,
    device_name: [libc::c_char; 256],
    pipeline_cache_uuid: [u8; 16],
    // Over-allocated tail buffer covering VkPhysicalDeviceLimits + SparseProperties on 64-bit
    _tail: [u64; 128],
}

fn c_char_buf_to_string(buf: &[libc::c_char]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let bytes = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), len) };
    String::from_utf8_lossy(bytes).trim().to_string()
}

fn probe_vulkan_gpu() -> Option<VulkanProbeResult> {
    let lib_name = CString::new("libvulkan.so").ok()?;
    let handle = unsafe { libc::dlopen(lib_name.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if handle.is_null() {
        return None;
    }

    let result = unsafe { query_vulkan_device(handle) };
    unsafe {
        libc::dlclose(handle);
    }
    result
}

unsafe fn resolve_sym<T>(handle: *mut libc::c_void, name: &str) -> Option<T>
where
    T: Copy,
{
    let cname = CString::new(name).ok()?;
    let sym = libc::dlsym(handle, cname.as_ptr());
    if sym.is_null() {
        None
    } else {
        Some(std::mem::transmute_copy(&sym))
    }
}

unsafe fn query_vulkan_device(handle: *mut libc::c_void) -> Option<VulkanProbeResult> {
    type VkCreateInstanceFn = unsafe extern "C" fn(
        *const VkInstanceCreateInfo,
        *const libc::c_void,
        *mut *mut libc::c_void,
    ) -> i32;
    type VkDestroyInstanceFn = unsafe extern "C" fn(*mut libc::c_void, *const libc::c_void);
    type VkEnumeratePhysicalDevicesFn =
        unsafe extern "C" fn(*mut libc::c_void, *mut u32, *mut *mut libc::c_void) -> i32;
    type VkGetPhysicalDevicePropertiesFn =
        unsafe extern "C" fn(*mut libc::c_void, *mut VkPhysicalDevicePropertiesRaw);

    let vk_create_instance: VkCreateInstanceFn = resolve_sym(handle, "vkCreateInstance")?;
    let vk_destroy_instance: VkDestroyInstanceFn = resolve_sym(handle, "vkDestroyInstance")?;
    let vk_enumerate_physical_devices: VkEnumeratePhysicalDevicesFn =
        resolve_sym(handle, "vkEnumeratePhysicalDevices")?;
    let vk_get_physical_device_properties: VkGetPhysicalDevicePropertiesFn =
        resolve_sym(handle, "vkGetPhysicalDeviceProperties")?;

    let app_name = CString::new("Voltix").ok()?;
    let app_info = VkApplicationInfo {
        s_type: 0, // VK_STRUCTURE_TYPE_APPLICATION_INFO
        p_next: ptr::null(),
        p_application_name: app_name.as_ptr(),
        application_version: 1,
        p_engine_name: app_name.as_ptr(),
        engine_version: 1,
        api_version: (1 << 22), // VK_API_VERSION_1_0 for universal instance compatibility
    };

    let create_info = VkInstanceCreateInfo {
        s_type: 1, // VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO
        p_next: ptr::null(),
        flags: 0,
        p_application_info: &app_info,
        enabled_layer_count: 0,
        pp_enabled_layer_names: ptr::null(),
        enabled_extension_count: 0,
        pp_enabled_extension_names: ptr::null(),
    };

    let mut instance: *mut libc::c_void = ptr::null_mut();
    if vk_create_instance(&create_info, ptr::null(), &mut instance) != 0 || instance.is_null() {
        return None;
    }

    let mut count: u32 = 0;
    let rc = vk_enumerate_physical_devices(instance, &mut count, ptr::null_mut());
    if rc != 0 || count == 0 {
        vk_destroy_instance(instance, ptr::null());
        return None;
    }

    let mut devices = vec![ptr::null_mut(); count.min(8) as usize];
    let mut fetch_count = devices.len() as u32;
    let rc = vk_enumerate_physical_devices(instance, &mut fetch_count, devices.as_mut_ptr());
    if rc != 0 && rc != 5 {
        // 5 = VK_INCOMPLETE
        vk_destroy_instance(instance, ptr::null());
        return None;
    }

    let dev = devices.into_iter().find(|d| !d.is_null());
    let Some(phys_dev) = dev else {
        vk_destroy_instance(instance, ptr::null());
        return None;
    };

    let mut props: VkPhysicalDevicePropertiesRaw = std::mem::zeroed();
    vk_get_physical_device_properties(phys_dev, &mut props);
    vk_destroy_instance(instance, ptr::null());

    let device_name = c_char_buf_to_string(&props.device_name);
    if device_name.is_empty() {
        return None;
    }

    let major = (props.api_version >> 22) & 0x7f;
    let minor = (props.api_version >> 12) & 0x3ff;
    let patch = props.api_version & 0xfff;
    let api_version_str = format!("{major}.{minor}.{patch}");

    let (vendor, vendor_name) = match props.vendor_id {
        0x5143 => (GpuVendor::Other, "Qualcomm".to_string()),
        0x13b5 => (GpuVendor::Other, "ARM".to_string()),
        0x1010 => (GpuVendor::Other, "Imagination".to_string()),
        0x10de => (GpuVendor::Nvidia, "NVIDIA".to_string()),
        0x1002 => (GpuVendor::Amd, "AMD".to_string()),
        0x8086 => (GpuVendor::Intel, "Intel".to_string()),
        _ => classify_gpu_vendor("", &device_name),
    };

    Some(VulkanProbeResult {
        device_name,
        vendor,
        vendor_name,
        api_version_str,
    })
}

struct EglProbeResult {
    renderer: String,
    vendor: String,
    version: String,
}

fn probe_egl_gpu() -> Option<EglProbeResult> {
    let egl_name = CString::new("libEGL.so").ok()?;
    let gles_name = CString::new("libGLESv2.so").ok()?;
    let egl_handle = unsafe { libc::dlopen(egl_name.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if egl_handle.is_null() {
        return None;
    }
    let gles_handle =
        unsafe { libc::dlopen(gles_name.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if gles_handle.is_null() {
        unsafe {
            libc::dlclose(egl_handle);
        }
        return None;
    }

    let res = unsafe { query_egl_strings(egl_handle, gles_handle) };
    unsafe {
        libc::dlclose(gles_handle);
        libc::dlclose(egl_handle);
    }
    res
}

unsafe fn query_egl_strings(
    egl_handle: *mut libc::c_void,
    gles_handle: *mut libc::c_void,
) -> Option<EglProbeResult> {
    type EglGetDisplayFn = unsafe extern "C" fn(*mut libc::c_void) -> *mut libc::c_void;
    type EglInitializeFn = unsafe extern "C" fn(*mut libc::c_void, *mut i32, *mut i32) -> u32;
    type EglChooseConfigFn = unsafe extern "C" fn(
        *mut libc::c_void,
        *const i32,
        *mut *mut libc::c_void,
        i32,
        *mut i32,
    ) -> u32;
    type EglCreatePbufferSurfaceFn =
        unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void, *const i32) -> *mut libc::c_void;
    type EglCreateContextFn = unsafe extern "C" fn(
        *mut libc::c_void,
        *mut libc::c_void,
        *mut libc::c_void,
        *const i32,
    ) -> *mut libc::c_void;
    type EglMakeCurrentFn = unsafe extern "C" fn(
        *mut libc::c_void,
        *mut libc::c_void,
        *mut libc::c_void,
        *mut libc::c_void,
    ) -> u32;
    type EglDestroySurfaceFn = unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void) -> u32;
    type EglDestroyContextFn = unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void) -> u32;
    type EglTerminateFn = unsafe extern "C" fn(*mut libc::c_void) -> u32;
    type GlGetStringFn = unsafe extern "C" fn(u32) -> *const libc::c_char;

    let egl_get_display: EglGetDisplayFn = resolve_sym(egl_handle, "eglGetDisplay")?;
    let egl_initialize: EglInitializeFn = resolve_sym(egl_handle, "eglInitialize")?;
    let egl_choose_config: EglChooseConfigFn = resolve_sym(egl_handle, "eglChooseConfig")?;
    let egl_create_pbuffer_surface: EglCreatePbufferSurfaceFn =
        resolve_sym(egl_handle, "eglCreatePbufferSurface")?;
    let egl_create_context: EglCreateContextFn = resolve_sym(egl_handle, "eglCreateContext")?;
    let egl_make_current: EglMakeCurrentFn = resolve_sym(egl_handle, "eglMakeCurrent")?;
    let egl_destroy_surface: EglDestroySurfaceFn = resolve_sym(egl_handle, "eglDestroySurface")?;
    let egl_destroy_context: EglDestroyContextFn = resolve_sym(egl_handle, "eglDestroyContext")?;
    let egl_terminate: EglTerminateFn = resolve_sym(egl_handle, "eglTerminate")?;
    let gl_get_string: GlGetStringFn = resolve_sym(gles_handle, "glGetString")?;

    let display = egl_get_display(ptr::null_mut());
    if display.is_null() {
        return None;
    }

    let mut major = 0_i32;
    let mut minor = 0_i32;
    if egl_initialize(display, &mut major, &mut minor) == 0 {
        return None;
    }

    const EGL_SURFACE_TYPE: i32 = 0x3033;
    const EGL_PBUFFER_BIT: i32 = 0x0001;
    const EGL_RENDERABLE_TYPE: i32 = 0x3040;
    const EGL_OPENGL_ES2_BIT: i32 = 0x0004;
    const EGL_NONE: i32 = 0x3038;
    const EGL_WIDTH: i32 = 0x3057;
    const EGL_HEIGHT: i32 = 0x3056;
    const EGL_CONTEXT_CLIENT_VERSION: i32 = 0x3098;

    let cfg_attrs = [
        EGL_SURFACE_TYPE,
        EGL_PBUFFER_BIT,
        EGL_RENDERABLE_TYPE,
        EGL_OPENGL_ES2_BIT,
        EGL_NONE,
    ];
    let mut config: *mut libc::c_void = ptr::null_mut();
    let mut num_cfg = 0_i32;
    if egl_choose_config(display, cfg_attrs.as_ptr(), &mut config, 1, &mut num_cfg) == 0
        || num_cfg == 0
        || config.is_null()
    {
        egl_terminate(display);
        return None;
    }

    let pbuf_attrs = [EGL_WIDTH, 1, EGL_HEIGHT, 1, EGL_NONE];
    let surface = egl_create_pbuffer_surface(display, config, pbuf_attrs.as_ptr());
    if surface.is_null() {
        egl_terminate(display);
        return None;
    }

    let ctx_attrs = [EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE];
    let context = egl_create_context(display, config, ptr::null_mut(), ctx_attrs.as_ptr());
    if context.is_null() {
        egl_destroy_surface(display, surface);
        egl_terminate(display);
        return None;
    }

    let mut out = None;
    if egl_make_current(display, surface, surface, context) != 0 {
        let renderer_ptr = gl_get_string(0x1F01); // GL_RENDERER
        let vendor_ptr = gl_get_string(0x1F00); // GL_VENDOR
        let version_ptr = gl_get_string(0x1F02); // GL_VERSION
        if !renderer_ptr.is_null() {
            let renderer = CStr::from_ptr(renderer_ptr)
                .to_string_lossy()
                .trim()
                .to_string();
            let vendor = if vendor_ptr.is_null() {
                String::new()
            } else {
                CStr::from_ptr(vendor_ptr)
                    .to_string_lossy()
                    .trim()
                    .to_string()
            };
            let version = if version_ptr.is_null() {
                String::new()
            } else {
                CStr::from_ptr(version_ptr)
                    .to_string_lossy()
                    .trim()
                    .to_string()
            };
            if !renderer.is_empty() {
                out = Some(EglProbeResult {
                    renderer,
                    vendor,
                    version,
                });
            }
        }
        egl_make_current(display, ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
    }

    egl_destroy_context(display, context);
    egl_destroy_surface(display, surface);
    egl_terminate(display);
    out
}

fn read_android_gpu_utilization() -> MetricValue<f64> {
    use crate::platform::gpu_probe::{kgsl_busy, read_metric};
    if cached_android_gpu_identity().vendor_name == "Qualcomm" {
        return read_metric("/sys/class/kgsl/kgsl-3d0/gpubusy", kgsl_busy);
    }
    // Mali sysfs formats are vendor-specific: a bare number may be a
    // utilization scale other than percent. Do not guess or clamp it.
    read_metric("/sys/class/misc/mali0/device/utilization", |_| {
        MetricValue::Unsupported("Mali utilization interface exists, but its units and sampling semantics are not verified for this driver".into())
    })
}

fn read_android_gpu_clock_mhz() -> MetricValue<u32> {
    use crate::platform::gpu_probe::read_metric;
    let mut failure = MetricValue::Unsupported(
        "No supported GPU clock interface exposed by device/driver".into(),
    );
    for (path, divisor) in [
        ("/sys/class/kgsl/kgsl-3d0/clock_mhz", 1),
        ("/sys/class/kgsl/kgsl-3d0/gpuclk", 1_000_000),
        (
            "/sys/class/misc/mali0/device/devfreq/mali0/cur_freq",
            1_000_000,
        ),
    ] {
        let result = read_metric(path, |raw| {
            match raw
                .trim()
                .parse::<u64>()
                .ok()
                .and_then(|hz| u32::try_from(hz / divisor).ok())
            {
                Some(mhz) if mhz > 0 => MetricValue::Available(mhz),
                Some(0) => MetricValue::Unavailable(
                    "Temporarily unavailable: GPU clock is zero (possibly idle)".into(),
                ),
                _ => MetricValue::Unavailable(format!(
                    "Sensor read failure: invalid GPU frequency in {path}"
                )),
            }
        });
        if result.is_available() {
            return result;
        }
        if !matches!(result, MetricValue::Unsupported(_)) {
            failure = result;
        }
    }
    failure
}

fn read_android_gpu_temperature() -> MetricValue<f64> {
    use crate::platform::gpu_probe::read_metric;
    let direct = read_metric("/sys/class/kgsl/kgsl-3d0/temp", |raw| {
        match raw.trim().parse::<i64>() {
            Ok(val) => {
                let celsius = if val > 200 {
                    val as f64 / 1000.0
                } else {
                    val as f64
                };
                if (0.0..=140.0).contains(&celsius) {
                    MetricValue::Available(celsius)
                } else {
                    MetricValue::Unavailable(
                        "Sensor read failure: GPU temperature out of range".into(),
                    )
                }
            }
            Err(_) => {
                MetricValue::Unavailable("Sensor read failure: invalid GPU temperature".into())
            }
        }
    });
    if direct.is_available() {
        return direct;
    }
    // GPU-specific zone names only. Never substitute CPU/SoC temperature.
    let thermal = read_android_thermal_zone(&["gpu", "gpuss"]).0;
    if thermal.is_available() || matches!(direct, MetricValue::Unsupported(_)) {
        thermal
    } else {
        direct
    }
}

pub fn collect_android_gpus() -> Vec<GpuTelemetry> {
    let identity = cached_android_gpu_identity();

    let platform = crate::platform::android_bridge::snapshot();
    let advanced = platform.shizuku.as_ref().map(|snapshot| {
        crate::platform::shizuku::measurements(snapshot, platform.foreground, platform.elapsed_ms)
    });
    let prefer = |normal: MetricValue<f64>, selected: Option<f64>| {
        if normal.is_available() {
            normal
        } else {
            selected.map(MetricValue::Available).unwrap_or(normal)
        }
    };
    let utilization = prefer(
        read_android_gpu_utilization(),
        advanced
            .as_ref()
            .and_then(|v| v.utilization.metric.copied()),
    );
    let temperature = prefer(
        read_android_gpu_temperature(),
        advanced
            .as_ref()
            .and_then(|v| v.temperature.metric.copied()),
    );
    let normal_clock = read_android_gpu_clock_mhz();
    let clock = if normal_clock.is_available() {
        normal_clock
    } else {
        advanced
            .as_ref()
            .and_then(|v| v.frequency.metric.copied())
            .map(|v| MetricValue::Available(v as u32))
            .unwrap_or(normal_clock)
    };
    let uma_reason = "Shared unified system memory (UMA); no dedicated VRAM".to_string();
    vec![GpuTelemetry {
        id: "android:gpu0".to_string(),
        name: identity.name.clone(),
        vendor: identity.vendor,
        vendor_name: identity.vendor_name.clone(),
        pci_slot: None,
        driver_version: identity.driver_version.clone(),
        utilization_percent: utilization,
        vram_total_bytes: MetricValue::Unsupported(uma_reason.clone()),
        vram_used_bytes: MetricValue::Unsupported(uma_reason.clone()),
        vram_free_bytes: MetricValue::Unsupported(uma_reason.clone()),
        vram_utilization_percent: MetricValue::Unsupported(uma_reason),
        temperature_celsius: temperature,
        core_clock_mhz: clock,
        memory_clock_mhz: MetricValue::Unsupported("Not exposed on Android".to_string()),
        power_draw_watts: MetricValue::Unsupported(
            "Not exposed via unprivileged sysfs".to_string(),
        ),
        power_limit_watts: MetricValue::Unsupported(
            "Not exposed via unprivileged sysfs".to_string(),
        ),
        fan_speed_percent: MetricValue::Unsupported("Not applicable".to_string()),
    }]
}

pub fn collect_android_storage() -> Vec<StorageDeviceTelemetry> {
    let candidates = [
        ("/data", "Internal Storage", "userdata"),
        ("/storage/emulated/0", "User Storage", "fuse/sdcardfs"),
    ];

    let mut seen_fsid: HashSet<(u64, u64)> = HashSet::new();
    let mut results = Vec::new();

    for (mount_point, device_label, fs_label) in candidates {
        let Ok(c_path) = CString::new(mount_point) else {
            continue;
        };
        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
        if rc != 0 {
            continue;
        }

        let block_size = if stat.f_frsize > 0 {
            stat.f_frsize as u64
        } else {
            stat.f_bsize as u64
        };
        let total = (stat.f_blocks as u64).saturating_mul(block_size);
        if total == 0 {
            continue;
        }
        let available = (stat.f_bavail as u64).saturating_mul(block_size);
        let dev_id = {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(mount_point).map(|m| m.dev()).unwrap_or(0)
        };
        let key = (total, dev_id);
        if !seen_fsid.insert(key) {
            continue;
        }

        let free_root = (stat.f_bfree as u64).saturating_mul(block_size);
        let used = total.saturating_sub(free_root);
        let pct = ((used as f64) / (total as f64)) * 100.0;

        results.push(StorageDeviceTelemetry {
            device: device_label.to_string(),
            mount_point: mount_point.to_string(),
            filesystem: fs_label.to_string(),
            total_bytes: MetricValue::Available(total),
            used_bytes: MetricValue::Available(used),
            available_bytes: MetricValue::Available(available),
            utilization_percent: MetricValue::Available(pct.clamp(0.0, 100.0)),
        });
    }

    results
}
