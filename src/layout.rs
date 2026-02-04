// Layout Calculator Module for Positioning Windows
// This module provides functions to calculate window layouts based on screen size and user preferences.

use crate::types::{ManagerType, DisplayInfo, WindowManagerConfig};
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS};


#[derive(Debug, Clone)]
pub struct WindowLayout {
    pub rect: RECT,
}

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
        vertical: bool,      // true = left/right, false = top/bottom
        ratio: f32,          // 0.0-1.0, ratio of first child
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
            BspNode::Split { first, second, .. } => {
                first.find_leaf(target_index)
                    .or_else(|| second.find_leaf(target_index))
            }
        }
    }

    // Insert a new window by splitting the most recent leaf (last inserted window)
    fn insert_window(&mut self, new_window_index: usize, gap: i32) {
        let last_window_index = new_window_index - 1;
        self.split_leaf_for_window(last_window_index, new_window_index, gap);
    }

    fn split_leaf_for_window(&mut self, target_index: usize, new_window_index: usize, gap: i32) -> bool {
        match self {
            BspNode::Leaf { window_index, rect } => {
                if *window_index == target_index {
                    // Found the leaf to split - decide direction based on aspect ratio
                    let width = rect.right - rect.left;
                    let height = rect.bottom - rect.top;
                    let split_vertical = width > height;
                    let split_ratio = 0.5;

                    if split_vertical {
                        // Split left/right
                        let split_x = rect.left + ((width as f32 * split_ratio) as i32);
                        
                        let first_rect = RECT {
                            left: rect.left,
                            top: rect.top,
                            right: split_x - gap / 2,
                            bottom: rect.bottom,
                        };
                        
                        let second_rect = RECT {
                            left: split_x + gap / 2,
                            top: rect.top,
                            right: rect.right,
                            bottom: rect.bottom,
                        };

                        *self = BspNode::Split {
                            vertical: true,
                            ratio: split_ratio,
                            first: Box::new(BspNode::new_leaf(*window_index, first_rect)),
                            second: Box::new(BspNode::new_leaf(new_window_index, second_rect)),
                        };
                    } else {
                        // Split top/bottom
                        let split_y = rect.top + ((height as f32 * split_ratio) as i32);
                        
                        let first_rect = RECT {
                            left: rect.left,
                            top: rect.top,
                            right: rect.right,
                            bottom: split_y - gap / 2,
                        };
                        
                        let second_rect = RECT {
                            left: rect.left,
                            top: split_y + gap / 2,
                            right: rect.right,
                            bottom: rect.bottom,
                        };

                        *self = BspNode::Split {
                            vertical: false,
                            ratio: split_ratio,
                            first: Box::new(BspNode::new_leaf(*window_index, first_rect)),
                            second: Box::new(BspNode::new_leaf(new_window_index, second_rect)),
                        };
                    }
                    true
                } else {
                    false
                }
            }
            BspNode::Split { first, second, .. } => {
                first.split_leaf_for_window(target_index, new_window_index, gap)
                    || second.split_leaf_for_window(target_index, new_window_index, gap)
            }
        }
    }
}

pub struct TilingLayout;

fn monitor_work_area() -> RECT {
    unsafe {
        let mut rect: RECT = std::mem::zeroed();
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut _ as _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        rect
    }
}

impl LayoutStrategy for TilingLayout {
    fn calculate_layout(
        &self,
        display: &DisplayInfo,
        window_count: usize,
        window_index: usize,
        config: &WindowManagerConfig,
    ) -> RECT {
        let gap = config.gap.unwrap_or(10) as i32;
        let work = monitor_work_area();

        let initial_rect = RECT {
            left: work.left + gap,
            top: work.top + gap,
            right: work.right - gap,
            bottom: work.bottom - gap,
        };

        // Start with first window taking full space
        let mut root = BspNode::new_leaf(0, initial_rect);

        // Insert each subsequent window by splitting the most recent leaf
        for i in 1..window_count {
            root.insert_window(i, gap);
        }

        // Find and return the rect for the requested window
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