// Layout Calculator Module for Positioning Windows
// This module provides functions to calculate window layouts based on screen size and user preferences.

use crate::types::{ManagerType, DisplayInfo, WindowManagerConfig};
use windows::Win32::Foundation::RECT;

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

pub struct TilingLayout;

impl LayoutStrategy for TilingLayout {
    fn calculate_layout(
        &self,
        display: &DisplayInfo,
        window_count: usize,
        window_index: usize,
        config: &WindowManagerConfig,
    ) -> RECT {
        let gap = config.gap.unwrap_or(10) as i32;
        let available_width = display.width - (gap * 2);
        let available_height = display.height - (gap * 2);

        // Simple two-column tiling: first window takes left half, rest stack on right
        let window_width = if window_index == 0 {
            available_width / 2
        } else {
            available_width / 2
        };

        let window_height = if window_index == 0 {
            available_height
        } else if window_count > 1 {
            available_height / (window_count - 1) as i32
        } else {
            available_height
        };

        let x = if window_index == 0 {
            display.x + gap
        } else {
            display.x + gap + window_width + gap
        };

        let y = if window_index == 0 {
            display.y + gap
        } else {
            display.y + gap + ((window_index - 1) as i32 * window_height)
        };

        RECT {
            left: x,
            top: y,
            right: x + window_width,
            bottom: y + window_height,
        }
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