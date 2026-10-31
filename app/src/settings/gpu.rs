use settings::{macros::define_settings_group, SupportedPlatforms, SyncToCloud};
use std::time::Duration;
use warpui::platform::GraphicsBackend;

define_settings_group!(GPUSettings, settings: [
   cpu_renderer: CpuRenderer {
       type: bool,
       default: false,
       supported_platforms: SupportedPlatforms::LINUX,
       sync_to_cloud: SyncToCloud::Never,
       private: false,
       toml_path: "system.cpu_renderer",
       description: "Whether to use the CPU renderer on Linux.",
   },
   prefer_low_power_gpu: PreferLowPowerGPU {
       type: bool,
       // Opt for the low power (integrated) GPU on Windows / Linux since discrete GPUs tend to be
        // more unstable.
       default: cfg!(any(target_os = "linux", target_os = "freebsd", windows)),
       supported_platforms: SupportedPlatforms::ALL,
       sync_to_cloud: SyncToCloud::Never,
       private: false,
       toml_path: "system.prefer_low_power_gpu",
       description: "Whether to prefer the integrated (low-power) GPU.",
   },
   preferred_backend: PreferredGraphicsBackend {
       type: Option<GraphicsBackend>,
       default: None,
       supported_platforms: SupportedPlatforms::WINDOWS,
       sync_to_cloud: SyncToCloud::Never,
       private: false,
       toml_path: "system.preferred_graphics_backend",
       description: "The preferred graphics backend on Windows.",
   },
]);

/// Default rate for repainting same-line spinner output while the CPU renderer is active.
pub const DEFAULT_CPU_TERMINAL_FPS: u64 = 15;

/// Whether the CPU (softbuffer) renderer is active, so terminal wakeups should be paced.
pub fn cpu_renderer_active() -> bool {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        warpui::rendering::cpu::cpu_renderer_requested()
    }
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    {
        false
    }
}

/// Shares `WARP_CPU_TERMINAL_FPS` with the window event loop's frame pacing.
pub fn cpu_terminal_frame_interval() -> Duration {
    fps_interval(
        std::env::var("WARP_CPU_TERMINAL_FPS").ok().as_deref(),
        DEFAULT_CPU_TERMINAL_FPS,
    )
}

fn fps_interval(value: Option<&str>, default_fps: u64) -> Duration {
    let fps = value
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|fps| *fps > 0)
        .unwrap_or(default_fps);

    Duration::from_micros(1_000_000 / fps)
}

#[cfg(test)]
#[path = "gpu_tests.rs"]
mod tests;
