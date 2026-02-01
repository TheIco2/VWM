use crate::config_yaml::{BarStyling, BarWidgets};

#[allow(dead_code)]
#[derive(Debug)]
pub struct LayoutRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct BarLayout {
    pub left: LayoutRect,
    pub center: LayoutRect,
    pub right: LayoutRect,
}

pub fn calculate_layout(
    width: i32,
    height: i32,
    style: &BarStyling,
    widgets: &BarWidgets, // needs widget counts
) -> BarLayout {
    let pad = &style.padding;

    // usable area after padding
    let usable_width = (width - pad._left - pad._right).max(0);
    let usable_height = (height - pad._top - pad._bottom).max(0);

    // widget counts
    let count_left = widgets.left.len() as i32;
    let count_center = widgets.center.len() as i32;
    let count_right = widgets.right.len() as i32;

    // total weight
    let total = count_left + count_center + count_right;

    // If no widgets => fallback to equal thirds
    let (w_left, w_center, w_right) = if total == 0 {
        (
            usable_width / 3,
            usable_width / 3,
            usable_width - 2 * (usable_width / 3),
        )
    } else {
        (
            // proportional distribution
            (usable_width * count_left / total),
            (usable_width * count_center / total),
            (usable_width * count_right / total),
        )
    };

    // compute segment start positions
    let left_x = pad._left;
    let center_x = left_x + w_left;
    let right_x = center_x + w_center;

    let left_rect = LayoutRect {
        left: left_x,
        top: pad._top,
        right: left_x + w_left,
        bottom: pad._top + usable_height,
    };

    let center_rect = LayoutRect {
        left: center_x,
        top: pad._top,
        right: center_x + w_center,
        bottom: pad._top + usable_height,
    };

    let right_rect = LayoutRect {
        left: right_x,
        top: pad._top,
        right: right_x + w_right,
        bottom: pad._top + usable_height,
    };

    BarLayout {
        left: left_rect,
        center: center_rect,
        right: right_rect,
    }
}
