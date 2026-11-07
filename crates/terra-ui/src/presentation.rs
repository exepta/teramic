use bevy::{
    prelude::Resource,
    render::{renderer::RenderAdapterInfo, settings::Backends},
    window::PresentMode,
};

/// Used only by explicit native A/B fixtures; normal gameplay uses adapter policy.
#[derive(Resource)]
pub struct PresentationOverride(pub PresentMode);

/// Keep VSync semantics while avoiding the measured RADV Immediate-acquire stalls.
pub fn preferred_present_mode(vsync: bool, adapter: Option<&RenderAdapterInfo>) -> PresentMode {
    choose(
        vsync,
        cfg!(target_os = "linux"),
        adapter.map(|info| {
            (
                info.vendor,
                Backends::from(info.backend) == Backends::VULKAN,
                info.driver.as_str(),
            )
        }),
    )
}

fn choose(vsync: bool, linux: bool, adapter: Option<(u32, bool, &str)>) -> PresentMode {
    if vsync {
        PresentMode::AutoVsync
    } else if linux
        && adapter.is_some_and(|(vendor, vulkan, driver)| {
            vendor == 0x1002 && vulkan && driver.eq_ignore_ascii_case("radv")
        })
    {
        // Bevy falls back to Immediate/Fifo if Mailbox is unavailable. This does
        // not add a frame limiter or change terrain/render quality.
        PresentMode::Mailbox
    } else {
        PresentMode::AutoNoVsync
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workaround_is_limited_to_the_measured_linux_radv_vulkan_path() {
        let radv = Some((0x1002, true, "radv"));
        assert_eq!(choose(false, true, radv), PresentMode::Mailbox);
        assert_eq!(choose(true, true, radv), PresentMode::AutoVsync);
        assert_eq!(choose(false, false, radv), PresentMode::AutoNoVsync);
        for other in [
            None,
            Some((0x10de, true, "radv")),
            Some((0x1002, false, "radv")),
            Some((0x1002, true, "amdvlk")),
        ] {
            assert_eq!(choose(false, true, other), PresentMode::AutoNoVsync);
            assert_eq!(choose(true, true, other), PresentMode::AutoVsync);
        }
    }
}
