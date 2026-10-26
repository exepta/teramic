use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct Frame {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl Frame {
    fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub fn style(self) -> String {
        format!(
            "left: {:.1}px; top: {:.1}px; width: {:.1}px; height: {:.1}px;",
            self.x, self.y, self.width, self.height
        )
    }
}

pub(super) struct Layout {
    pub width: f32,
    pub height: f32,
    pub compact: bool,
    pub tiny: bool,
    pub art: Frame,
    pub portrait: Frame,
    pub follow: Frame,
    pub rest: Frame,
    pub free: Frame,
    pub caption: Frame,
    pub follow_info: Frame,
    pub rest_info: Frame,
    pub none_info: Frame,
}
impl Layout {
    pub fn new(canvas: Vec2, viewport: Vec2) -> Self {
        let available = (canvas - Vec2::splat(24.0)).max(Vec2::ONE);
        let wide_side = (available.x / 1.72)
            .min((available.y - 24.0) / 1.10)
            .min(720.0);
        // Automatic UI fitting preserves a large virtual canvas on small windows.
        let compact = wide_side < 500.0 || viewport.x < 1000.0 || viewport.y < 600.0;
        let (side, left, top, width, height, follow_info, rest_info, none_info) = if compact {
            let side = available
                .x
                .min((available.y - 180.0).max(1.0))
                .min(500.0 * (canvas.x / viewport.x.max(1.0)).max(1.0));
            let top = 68.0;
            (
                side,
                0.0,
                top,
                side,
                side + 180.0,
                Frame::new(side * 0.14, 0.0, side * 0.72, 64.0),
                Frame::new(0.0, top + side + 4.0, side * 0.48, 88.0),
                Frame::new(side * 0.52, top + side + 4.0, side * 0.48, 88.0),
            )
        } else {
            let side = wide_side;
            (
                side,
                side * 0.36,
                side * 0.10,
                side * 1.72,
                side * 1.10 + 24.0,
                Frame::new(side * 0.62, 0.0, side * 0.48, side * 0.14),
                Frame::new(0.0, side * 0.53, side * 0.35, side * 0.20),
                Frame::new(side * 1.37, side * 0.53, side * 0.35, side * 0.20),
            )
        };
        let at = |x: f32, y: f32, w: f32, h: f32| {
            Frame::new(left + side * x, top + side * y, side * w, side * h)
        };
        Self {
            width,
            height,
            compact,
            tiny: side < 240.0,
            art: at(0.0, 0.0, 1.0, 1.0),
            portrait: at(0.275, 0.36, 0.45, 0.45),
            follow: at(0.265, 0.045, 0.47, 0.31),
            rest: at(0.016, 0.444, 0.226, 0.226),
            free: at(0.758, 0.444, 0.226, 0.226),
            caption: at(0.31, 0.815, 0.38, 0.13),
            follow_info,
            rest_info,
            none_info,
        }
    }
    pub fn style(&self) -> String {
        format!("width: {:.1}px; height: {:.1}px;", self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn overlaps(a: Frame, b: Frame) -> bool {
        a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
    }
    #[test]
    fn radial_anchors_and_description_plaques_fit_desktop_and_compact_windows() {
        for (canvas, scale) in [
            (Vec2::new(1920.0, 1080.0), 1.0),
            (Vec2::new(1280.0, 720.0), 720.0 / 740.0),
            (Vec2::new(800.0, 600.0), 0.8),
            (Vec2::new(320.0, 640.0), 0.32),
            (Vec2::new(640.0, 360.0), 360.0 / 740.0),
        ] {
            let layout = Layout::new(canvas / scale, canvas);
            assert!(
                layout.width * scale <= canvas.x && layout.height * scale <= canvas.y,
                "{canvas:?}"
            );
            let frames = [
                layout.follow,
                layout.rest,
                layout.free,
                layout.caption,
                layout.follow_info,
                layout.rest_info,
                layout.none_info,
            ];
            for (index, frame) in frames.iter().enumerate() {
                assert!(frame.x >= 0.0 && frame.y >= 0.0);
                assert!(frame.x + frame.width <= layout.width + 0.01);
                assert!(frame.y + frame.height <= layout.height + 0.01);
                for other in &frames[index + 1..] {
                    assert!(
                        !overlaps(*frame, *other),
                        "{canvas:?}: {frame:?} / {other:?}"
                    );
                }
            }
            for button in [layout.follow, layout.rest, layout.free] {
                assert!(!overlaps(button, layout.portrait));
                assert!(
                    button.width * scale >= 44.0 && button.height * scale >= 44.0,
                    "{canvas:?}"
                );
            }
        }
    }
    #[test]
    fn reference_layout_keeps_the_portrait_centered_and_uses_one_stable_frame() {
        let viewport = Vec2::new(1920.0, 1080.0);
        let layout = Layout::new(viewport, viewport);
        assert!(!layout.compact);
        assert!(
            (layout.portrait.x + layout.portrait.width * 0.5 - layout.width * 0.5).abs() < 0.01
        );
        assert!((layout.follow.x + layout.follow.width * 0.5 - layout.width * 0.5).abs() < 0.01);
        assert!(layout.rest.x < layout.portrait.x);
        assert!(layout.free.x > layout.portrait.x + layout.portrait.width);
        assert!(layout.caption.y > layout.portrait.y + layout.portrait.height);
        assert_eq!(layout.style(), Layout::new(viewport, viewport).style());
    }

    #[test]
    fn fitted_ui_scale_does_not_hide_the_actual_narrow_window_from_the_radial_layout() {
        let viewport = Vec2::new(640.0, 480.0);
        let scale = 0.64;
        let layout = Layout::new(viewport / scale, viewport);
        assert!(layout.compact);
        assert!(layout.width * scale <= viewport.x);
        assert!(layout.height * scale <= viewport.y);
        assert!(layout.rest.width * scale >= 44.0);
        assert!(layout.follow_info.height * scale >= 36.0);
    }

    #[test]
    fn high_dpi_logical_size_does_not_turn_a_wide_physical_window_into_a_compact_layout() {
        let physical = Vec2::new(1280.0, 800.0);
        let logical = physical / 1.5;
        let ui_scale = logical.y / 740.0;
        let layout = Layout::new(logical / ui_scale, physical);
        assert!(!layout.compact);
        let physical_scale = ui_scale * 1.5;
        assert!(layout.width * physical_scale <= physical.x);
        assert!(layout.height * physical_scale <= physical.y);
    }
}
