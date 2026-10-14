//! Pure rectangle math for the docked panel. All values are physical pixels.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Left,
    Right,
}

/// DIPs to physical pixels at the given DPI, rounded like Win32 `MulDiv`.
pub fn to_px(dip: u32, dpi: u32) -> i32 {
    ((dip as u64 * dpi as u64 + 48) / 96) as i32
}

/// The panel's width can be dragged within this range (DIP).
pub fn clamp_width(dip: u32) -> u32 {
    dip.clamp(320, 720)
}

/// Shrinks `area` to a `width`-wide strip against `edge`. Used for the initial proposal
/// and again after `ABM_QUERYPOS`, which may move the docked edge but not our width.
pub fn dock(area: Rect, edge: Edge, width: i32) -> Rect {
    match edge {
        Edge::Right => Rect { left: area.right - width, ..area },
        Edge::Left => Rect { right: area.left + width, ..area },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITOR: Rect = Rect { left: 0, top: 0, right: 1920, bottom: 1200 };

    #[test]
    fn dip_to_px_at_common_scales() {
        assert_eq!(to_px(380, 96), 380); // 100%
        assert_eq!(to_px(380, 120), 475); // 125%
        assert_eq!(to_px(380, 144), 570); // 150%
        assert_eq!(to_px(380, 168), 665); // 175%
    }

    #[test]
    fn clamps_width() {
        assert_eq!(clamp_width(100), 320);
        assert_eq!(clamp_width(500), 500);
        assert_eq!(clamp_width(5000), 720);
    }

    #[test]
    fn docks_to_either_edge() {
        let right = dock(MONITOR, Edge::Right, 475);
        assert_eq!(right, Rect { left: 1445, top: 0, right: 1920, bottom: 1200 });
        let left = dock(MONITOR, Edge::Left, 475);
        assert_eq!(left, Rect { left: 0, top: 0, right: 475, bottom: 1200 });
    }

    #[test]
    fn keeps_width_when_shell_moves_the_edge() {
        // Another AppBar already occupies the right 100 px.
        let adjusted = Rect { right: 1820, ..dock(MONITOR, Edge::Right, 475) };
        let fixed = dock(adjusted, Edge::Right, 475);
        assert_eq!((fixed.left, fixed.right, fixed.width()), (1345, 1820, 475));
    }

    #[test]
    fn works_on_a_secondary_monitor() {
        let second = Rect { left: -1920, top: 0, right: 0, bottom: 1080 };
        assert_eq!(dock(second, Edge::Left, 380).right, -1540);
    }
}
