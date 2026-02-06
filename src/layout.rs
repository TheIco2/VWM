// ~/src/layout.rs
// Layout Calculator Module for Positioning Windows
// This module provides functions to calculate window layouts based on screen size and user preferences.

use crate::types::{ManagerType, DisplayInfo, WindowManagerConfig};
use crate::config::styling::GapBehavior;
use windows::Win32::Foundation::RECT;

pub trait LayoutStrategy {
    fn calculate_layout(
        &self,
        display: &DisplayInfo,
        window_count: usize,
        window_index: usize,
        config: &WindowManagerConfig,
    ) -> RECT;
}

// BSP (Binary Space Partitioning) Node for recursive layout
#[derive(Debug, Clone)]
enum BspNode {
    Leaf {
        window_index: usize,
        rect: RECT,
    },
    Split {
        first: Box<BspNode>,
        second: Box<BspNode>,
    },
}

impl BspNode {
    fn new_leaf(window_index: usize, rect: RECT) -> Self {
        BspNode::Leaf { window_index, rect }
    }

    // Find the leaf node for a specific window index
    fn find_leaf(&self, target_index: usize) -> Option<RECT> {
        match self {
            BspNode::Leaf { window_index, rect } => {
                if *window_index == target_index {
                    Some(*rect)
                } else {
                    None
                }
            }
            BspNode::Split { first, second } => {
                first.find_leaf(target_index)
                    .or_else(|| second.find_leaf(target_index))
            }
        }
    }

    // Insert a new window by splitting the most recent leaf (last inserted window)
    fn insert_window(&mut self, new_window_index: usize, gap: i32, behavior: &GapBehavior) {
        let last_window_index = new_window_index - 1;
        self.split_leaf_for_window(last_window_index, new_window_index, gap, behavior);
    }


    fn split_leaf_for_window(
        &mut self,
        target_index: usize,
        new_window_index: usize,
        gap: i32,
        behavior: &GapBehavior,
    ) -> bool {
        match self {
            BspNode::Leaf { window_index, rect } => {
                if *window_index == target_index {
                    let width = rect.right - rect.left;
                    let height = rect.bottom - rect.top;
                    let split_vertical = width > height;

                    let half = gap / 2;

                    if split_vertical {
                        let split_x = rect.left + width / 2;

                        let (left_gap, right_gap) = match behavior {
                            GapBehavior::PerWindow => (gap, gap),
                            GapBehavior::Shared => (half, half),
                        };

                        let first_rect = RECT {
                            left: rect.left,
                            top: rect.top,
                            right: split_x - right_gap,
                            bottom: rect.bottom,
                        };

                        let second_rect = RECT {
                            left: split_x + left_gap,
                            top: rect.top,
                            right: rect.right,
                            bottom: rect.bottom,
                        };

                        *self = BspNode::Split {
                            first: Box::new(BspNode::new_leaf(*window_index, first_rect)),
                            second: Box::new(BspNode::new_leaf(new_window_index, second_rect)),
                        };
                    } else {
                        let split_y = rect.top + height / 2;

                        let (top_gap, bottom_gap) = match behavior {
                            GapBehavior::PerWindow => (gap, gap),
                            GapBehavior::Shared => (half, half),
                        };

                        let first_rect = RECT {
                            left: rect.left,
                            top: rect.top,
                            right: rect.right,
                            bottom: split_y - bottom_gap,
                        };

                        let second_rect = RECT {
                            left: rect.left,
                            top: split_y + top_gap,
                            right: rect.right,
                            bottom: rect.bottom,
                        };

                        *self = BspNode::Split {
                            first: Box::new(BspNode::new_leaf(*window_index, first_rect)),
                            second: Box::new(BspNode::new_leaf(new_window_index, second_rect)),
                        };
                    }

                    true
                } else {
                    false
                }
            }
            BspNode::Split { first, second } => {
                first.split_leaf_for_window(target_index, new_window_index, gap, behavior)
                    || second.split_leaf_for_window(target_index, new_window_index, gap, behavior)
            }
        }
    }

}

pub struct TilingLayout;

impl LayoutStrategy for TilingLayout {
    fn calculate_layout(
        &self,
        display: &DisplayInfo,
        window_count: usize,
        window_index: usize,
        config: &WindowManagerConfig,
    ) -> RECT {        
        let gap_cfg = config.styling
            .as_ref()
            .and_then(|s| s.gap.as_ref());

        let gap = gap_cfg.map(|g| g.space as i32).unwrap_or(0);
        let behavior = gap_cfg
            .map(|g| &g.behavior)
            .unwrap_or(&GapBehavior::PerWindow);

        // Edge gaps are always full gap (perimeter is not shared)
        let edge_gap = gap;

        let initial_rect = RECT {
            left: display.x + edge_gap,
            top: display.y + edge_gap,
            right: display.x + display.width - edge_gap,
            bottom: display.y + display.height - edge_gap,
        };

        let mut root = BspNode::new_leaf(0, initial_rect);

        for i in 1..window_count {
            root.insert_window(i, gap, behavior);
        }

        root.find_leaf(window_index).unwrap_or(initial_rect)
    }
}

pub struct FloatingLayout;

impl LayoutStrategy for FloatingLayout {
    fn calculate_layout(
        &self,
        display: &DisplayInfo,
        _window_count: usize,
        _window_index: usize,
        _config: &WindowManagerConfig,
    ) -> RECT {
        // Floating layout: windows stay at their current positions
        // Return the full display area as a suggestion
        RECT {
            left: display.x,
            top: display.y,
            right: display.x + display.width,
            bottom: display.y + display.height,
        }
    }
}

pub struct StackingLayout;

impl LayoutStrategy for StackingLayout {
    fn calculate_layout(
        &self,
        display: &DisplayInfo,
        _window_count: usize,
        _window_index: usize,
        _config: &WindowManagerConfig,
    ) -> RECT {
        // Stacking layout: all windows maximized but offset slightly
        let offset = 30;
        RECT {
            left: display.x + offset,
            top: display.y + offset,
            right: display.x + display.width - offset,
            bottom: display.y + display.height - offset,
        }
    }
}

pub fn get_layout_strategy(manager_type: ManagerType) -> Box<dyn LayoutStrategy> {
    match manager_type {
        ManagerType::Tiling => Box::new(TilingLayout),
        ManagerType::Floating => Box::new(FloatingLayout),
        ManagerType::Stacking => Box::new(StackingLayout),
    }
}