/// Window resize state management and delta application
/// Handles tracking and applying resize operations during tiling

use crate::{
    types::{DisplayInfo, WindowManagerConfig, ManagerType},
    config::FiltersConfig,
    layout::LayoutStrategy,
    info, DEBUG_NAME,
};

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

use std::{mem, collections::{HashMap, HashSet}};

use super::ops_utils::{
    RESIZE_STATE, EDGE_TOLERANCE, ResizeIntent,
    enumerate_windows_with_all, compute_layout_targets, get_layout_bounds,
};
use super::window_analysis::{find_neighbor_indices, find_aligned_indices};
use super::ops_utils::get_internal_gap;
use super::positioning::get_window_title;

const DEBUG_SUBTAG: &str = "RESIZE_OPS";

/// Update the resize intent for a window during tiling
pub fn update_resize_state_for_window(
    display: &DisplayInfo,
    all_displays: &[DisplayInfo],
    config: &WindowManagerConfig,
    layout_strategy: &dyn LayoutStrategy,
    hwnd: HWND,
) -> bool {
    if config.manager_type.unwrap_or_default() != ManagerType::Tiling {
        return false;
    }

    let default_filters = FiltersConfig::default();
    let filters = config.filters.as_ref().unwrap_or(&default_filters);
    let windows = enumerate_windows_with_all(display, all_displays, filters);
    if windows.is_empty() {
        return false;
    }

    // CRITICAL: Compare against layout WITH existing resize deltas applied
    // This prevents detecting false resizes when windows are affected by other windows' persistent resize intents
    let mut targets_with_deltas = compute_layout_targets(&windows, display, config, layout_strategy);
    
    // Apply existing resize deltas to get the expected position including effects of other windows' resizes
    apply_resize_deltas(display, config, &mut targets_with_deltas);

    let idx = targets_with_deltas.iter().position(|(h, _)| *h == hwnd);
    let Some(idx) = idx else { return false; };
    let expected = targets_with_deltas[idx].1;

    let actual = unsafe {
        let mut rect: RECT = mem::zeroed();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }
        rect
    };

    let mut intent = ResizeIntent::default();

    // Use a larger tolerance to avoid fighting with user - only capture significant resizes
    let resize_threshold = 10; // Increased to 10px to avoid false positives from WM adjustments

    let dl = actual.left - expected.left;
    let dr = actual.right - expected.right;
    
    // Only detect resize if the SIZE changed significantly, not just position
    let width_change = (actual.right - actual.left) - (expected.right - expected.left);
    let height_change = (actual.bottom - actual.top) - (expected.bottom - expected.top);
    
    // Detect horizontal resize with better precision
    if dl.abs() >= resize_threshold || dr.abs() >= resize_threshold {
        // If both edges moved significantly in opposite directions, it's ambiguous - prefer the larger delta
        if dl.abs() > resize_threshold && dr.abs() <= resize_threshold {
            intent.left = Some(actual.left);
        } else if dr.abs() > resize_threshold && dl.abs() <= resize_threshold {
            intent.right = Some(actual.right);
        } else if dl.abs() > resize_threshold && dr.abs() > resize_threshold {
            // Both edges moved - only count as resize if width actually changed
            if width_change.abs() >= resize_threshold {
                if dl.abs() > dr.abs() {
                    intent.left = Some(actual.left);
                } else {
                    intent.right = Some(actual.right);
                }
            }
        }
    }

    let dt = actual.top - expected.top;
    let db = actual.bottom - expected.bottom;
    
    // Detect vertical resize with better precision
    if dt.abs() >= resize_threshold || db.abs() >= resize_threshold {
        if dt.abs() > resize_threshold && db.abs() <= resize_threshold {
            intent.top = Some(actual.top);
        } else if db.abs() > resize_threshold && dt.abs() <= resize_threshold {
            intent.bottom = Some(actual.bottom);
        } else if dt.abs() > resize_threshold && db.abs() > resize_threshold {
            // Both edges moved - only count as resize if height actually changed
            if height_change.abs() >= resize_threshold {
                if dt.abs() > db.abs() {
                    intent.top = Some(actual.top);
                } else {
                    intent.bottom = Some(actual.bottom);
                }
            }
        }
    }

    if intent.left.is_none()
        && intent.right.is_none()
        && intent.top.is_none()
        && intent.bottom.is_none()
    {
        return false;
    }

    let bounds = get_layout_bounds(display, config);
    let internal_gap = get_internal_gap(config);

    // Don't allow resizing edges that are at monitor boundaries
    if expected.left <= bounds.left + EDGE_TOLERANCE {
        intent.left = None;
    }
    if expected.right >= bounds.right - EDGE_TOLERANCE {
        intent.right = None;
    }
    if expected.top <= bounds.top + EDGE_TOLERANCE {
        intent.top = None;
    }
    if expected.bottom >= bounds.bottom - EDGE_TOLERANCE {
        intent.bottom = None;
    }

    // Don't allow resizing edges that have no neighbors (nothing to push)
    if intent.right.is_some() && find_neighbor_indices(&targets_with_deltas, idx, "right", internal_gap).is_empty() {
        intent.right = None;
    }
    if intent.left.is_some() && find_neighbor_indices(&targets_with_deltas, idx, "left", internal_gap).is_empty() {
        intent.left = None;
    }
    if intent.top.is_some() && find_neighbor_indices(&targets_with_deltas, idx, "top", internal_gap).is_empty() {
        intent.top = None;
    }
    if intent.bottom.is_some() && find_neighbor_indices(&targets_with_deltas, idx, "bottom", internal_gap).is_empty() {
        intent.bottom = None;
    }

    if intent.left.is_none()
        && intent.right.is_none()
        && intent.top.is_none()
        && intent.bottom.is_none()
    {
        return false;
    }

    // Log resize intent for debugging
    let title = get_window_title(hwnd);
    let title_short = if title.chars().count() > 30 {
        format!("{}...", title.chars().take(27).collect::<String>())
    } else {
        title
    };
    
    info!(
        "[{}][{}] Resize '{}': L:{} R:{} T:{} B:{} | ΔW:{:+} ΔH:{:+}",
        DEBUG_NAME, DEBUG_SUBTAG, title_short,
        intent.left.map(|v| format!("{:+}", v - expected.left)).unwrap_or_else(|| "—".to_string()),
        intent.right.map(|v| format!("{:+}", v - expected.right)).unwrap_or_else(|| "—".to_string()),
        intent.top.map(|v| format!("{:+}", v - expected.top)).unwrap_or_else(|| "—".to_string()),
        intent.bottom.map(|v| format!("{:+}", v - expected.bottom)).unwrap_or_else(|| "—".to_string()),
        width_change,
        height_change
    );

    let state = RESIZE_STATE
        .get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())))
        .clone();
    let mut state_map = state.lock().unwrap();
    let monitor_state = state_map.entry(display.id.clone()).or_insert_with(HashMap::new);
    
    // Store intent for the resized window, preserving other-axis intents
    let hwnd_key = hwnd.0 as isize;
    let mut merged_intent = monitor_state.get(&hwnd_key).copied().unwrap_or_default();

    if intent.left.is_some() {
        merged_intent.left = intent.left;
        merged_intent.right = None;
    } else if intent.right.is_some() {
        merged_intent.right = intent.right;
        merged_intent.left = None;
    }

    if intent.top.is_some() {
        merged_intent.top = intent.top;
        merged_intent.bottom = None;
    } else if intent.bottom.is_some() {
        merged_intent.bottom = intent.bottom;
        merged_intent.top = None;
    }

    monitor_state.insert(hwnd_key, merged_intent);
    
    // CRITICAL: Clear conflicting resize intents from neighbor windows
    // When resizing an edge, neighbors on that edge should have their opposing edge cleared
    // This prevents conflicting intents on the same physical edge
    
    if intent.left.is_some() {
        let neighbors = find_neighbor_indices(&targets_with_deltas, idx, "left", internal_gap);
        for &neighbor_idx in &neighbors {
            let neighbor_hwnd = targets_with_deltas[neighbor_idx].0;
            if let Some(neighbor_intent) = monitor_state.get_mut(&(neighbor_hwnd.0 as isize)) {
                neighbor_intent.right = None; // Clear opposing edge
            }
        }
    }
    
    if intent.right.is_some() {
        let neighbors = find_neighbor_indices(&targets_with_deltas, idx, "right", internal_gap);
        for &neighbor_idx in &neighbors {
            let neighbor_hwnd = targets_with_deltas[neighbor_idx].0;
            if let Some(neighbor_intent) = monitor_state.get_mut(&(neighbor_hwnd.0 as isize)) {
                neighbor_intent.left = None; // Clear opposing edge
            }
        }
    }
    
    if intent.top.is_some() {
        let neighbors = find_neighbor_indices(&targets_with_deltas, idx, "top", internal_gap);
        for &neighbor_idx in &neighbors {
            let neighbor_hwnd = targets_with_deltas[neighbor_idx].0;
            if let Some(neighbor_intent) = monitor_state.get_mut(&(neighbor_hwnd.0 as isize)) {
                neighbor_intent.bottom = None; // Clear opposing edge
            }
        }
    }
    
    if intent.bottom.is_some() {
        let neighbors = find_neighbor_indices(&targets_with_deltas, idx, "bottom", internal_gap);
        for &neighbor_idx in &neighbors {
            let neighbor_hwnd = targets_with_deltas[neighbor_idx].0;
            if let Some(neighbor_intent) = monitor_state.get_mut(&(neighbor_hwnd.0 as isize)) {
                neighbor_intent.top = None; // Clear opposing edge
            }
        }
    }
    
    // CRITICAL: Propagate resize intent to all windows that share the same edge
    // Windows that share an edge must move together as a rigid unit
    // Otherwise we get conflicting intents that create impossible layouts
    
    if let Some(left_pos) = intent.left {
        let aligned = find_aligned_indices(&targets_with_deltas, idx, "left");
        for &aligned_idx in &aligned {
            if aligned_idx != idx {
                let aligned_hwnd = targets_with_deltas[aligned_idx].0;
                let aligned_state = monitor_state.entry(aligned_hwnd.0 as isize).or_insert_with(ResizeIntent::default);
                aligned_state.left = Some(left_pos);
            }
        }
    }
    
    if let Some(right_pos) = intent.right {
        let aligned = find_aligned_indices(&targets_with_deltas, idx, "right");
        for &aligned_idx in &aligned {
            if aligned_idx != idx {
                let aligned_hwnd = targets_with_deltas[aligned_idx].0;
                let aligned_state = monitor_state.entry(aligned_hwnd.0 as isize).or_insert_with(ResizeIntent::default);
                aligned_state.right = Some(right_pos);
            }
        }
    }
    
    if let Some(top_pos) = intent.top {
        let aligned = find_aligned_indices(&targets_with_deltas, idx, "top");
        for &aligned_idx in &aligned {
            if aligned_idx != idx {
                let aligned_hwnd = targets_with_deltas[aligned_idx].0;
                let aligned_state = monitor_state.entry(aligned_hwnd.0 as isize).or_insert_with(ResizeIntent::default);
                aligned_state.top = Some(top_pos);
            }
        }
    }
    
    if let Some(bottom_pos) = intent.bottom {
        let aligned = find_aligned_indices(&targets_with_deltas, idx, "bottom");
        for &aligned_idx in &aligned {
            if aligned_idx != idx {
                let aligned_hwnd = targets_with_deltas[aligned_idx].0;
                let aligned_state = monitor_state.entry(aligned_hwnd.0 as isize).or_insert_with(ResizeIntent::default);
                aligned_state.bottom = Some(bottom_pos);
            }
        }
    }

    true
}

/// Apply accumulated resize deltas to window rectangles
pub fn apply_resize_deltas(
    display: &DisplayInfo,
    config: &WindowManagerConfig,
    rects: &mut Vec<(HWND, RECT)>,
) {
    if config.manager_type.unwrap_or_default() != ManagerType::Tiling {
        return;
    }

    let internal_gap = get_internal_gap(config);

    let state = RESIZE_STATE
        .get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())))
        .clone();
    let mut state_map = state.lock().unwrap();
    let monitor_state = match state_map.get_mut(&display.id) {
        Some(state) => state,
        None => return,
    };


    monitor_state.retain(|hwnd, _| rects.iter().any(|(h, _)| h.0 as isize == *hwnd));

    let mut deltas: HashMap<usize, (i32, i32, i32, i32)> = HashMap::new();
    let mut ops: Vec<(Vec<usize>, Vec<usize>, i32, &'static str)> = Vec::new();
    let mut processed_edges: HashSet<(i32, &'static str)> = HashSet::new();

    info!("[RESIZE] Processing resize intents for {} windows:", monitor_state.len());
    for (hwnd, intent) in monitor_state.iter() {
        info!("  HWND={} - L:{:?} R:{:?} T:{:?} B:{:?}", 
            hwnd, intent.left, intent.right, intent.top, intent.bottom);
    }

    for idx in 0..rects.len() {
        let hwnd_val = (rects[idx].0).0 as isize;
        let Some(intent) = monitor_state.get(&hwnd_val).copied() else {
            continue;
        };

        if let Some(target_right) = intent.right {
            // Skip if we've already processed this edge position
            if processed_edges.insert((target_right, "right")) {
                let delta = target_right - rects[idx].1.right;
                if delta != 0 {
                    let same = find_aligned_indices(rects, idx, "right");
                    let neighbors = find_neighbor_indices(rects, idx, "right", internal_gap);
                    info!("[RESIZE] idx={} RIGHT delta={} (target={} current={}) same={:?} neighbors={:?}", 
                        idx, delta, target_right, rects[idx].1.right, same, neighbors);
                    ops.push((same, neighbors, delta, "right"));
                }
            }
        }

        if let Some(target_left) = intent.left {
            if processed_edges.insert((target_left, "left")) {
                let delta = target_left - rects[idx].1.left;
                if delta != 0 {
                    let same = find_aligned_indices(rects, idx, "left");
                    let neighbors = find_neighbor_indices(rects, idx, "left", internal_gap);
                    info!("[RESIZE] idx={} LEFT delta={} (target={} current={}) same={:?} neighbors={:?}", 
                        idx, delta, target_left, rects[idx].1.left, same, neighbors);
                    ops.push((same, neighbors, delta, "left"));
                }
            }
        }

        if let Some(target_bottom) = intent.bottom {
            if processed_edges.insert((target_bottom, "bottom")) {
                let delta = target_bottom - rects[idx].1.bottom;
                if delta != 0 {
                    let same = find_aligned_indices(rects, idx, "bottom");
                    let neighbors = find_neighbor_indices(rects, idx, "bottom", internal_gap);
                    info!("[RESIZE] idx={} BOTTOM delta={} (target={} current={}) same={:?} neighbors={:?}", 
                        idx, delta, target_bottom, rects[idx].1.bottom, same, neighbors);
                    ops.push((same, neighbors, delta, "bottom"));
                }
            }
        }

        if let Some(target_top) = intent.top {
            if processed_edges.insert((target_top, "top")) {
                let delta = target_top - rects[idx].1.top;
                if delta != 0 {
                    let same = find_aligned_indices(rects, idx, "top");
                    let neighbors = find_neighbor_indices(rects, idx, "top", internal_gap);
                    info!("[RESIZE] idx={} TOP delta={} (target={} current={}) same={:?} neighbors={:?}", 
                        idx, delta, target_top, rects[idx].1.top, same, neighbors);
                    ops.push((same, neighbors, delta, "top"));
                }
            }
        }
    }

    // Apply deltas to windows
    // Delta tuple format: (left_delta, right_delta, top_delta, bottom_delta)
    for (same, neighbors, delta, side) in ops {
        match side {
            "right" => {
                // Windows with same right edge all move together
                for n in same {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.1 += delta; // Adjust right edge
                }
                // Neighbors to the right get their left edge pushed (resized, not moved)
                for n in neighbors {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.0 += delta; // Adjust left edge only - this resizes the neighbor
                }
            }
            "left" => {
                // Windows with same left edge all move together
                for n in same {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.0 += delta; // Adjust left edge
                }
                // Neighbors to the left get their right edge pushed (resized, not moved)
                for n in neighbors {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.1 += delta; // Adjust right edge only - this resizes the neighbor
                }
            }
            "bottom" => {
                // Windows with same bottom edge all move together
                for n in same {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.3 += delta; // Adjust bottom edge
                }
                // Neighbors below get their top edge pushed (resized, not moved)
                for n in neighbors {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.2 += delta; // Adjust top edge only - this resizes the neighbor
                }
            }
            "top" => {
                // Windows with same top edge all move together
                for n in same {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.2 += delta; // Adjust top edge
                }
                // Neighbors above get their bottom edge pushed (resized, not moved)
                for n in neighbors {
                    let entry = deltas.entry(n).or_insert((0, 0, 0, 0));
                    entry.3 += delta; // Adjust bottom edge only - this resizes the neighbor
                }
            }
            _ => {}
        }
    }

    info!("[RESIZE] Applying final deltas to {} windows:", deltas.len());
    for (idx, (left_delta, right_delta, top_delta, bottom_delta)) in deltas {
        info!("  idx={} L:{:+} R:{:+} T:{:+} B:{:+} -> New size: {}x{}", 
            idx, left_delta, right_delta, top_delta, bottom_delta,
            (rects[idx].1.right + right_delta) - (rects[idx].1.left + left_delta),
            (rects[idx].1.bottom + bottom_delta) - (rects[idx].1.top + top_delta));
        
        if left_delta != 0 {
            rects[idx].1.left += left_delta;
        }
        if right_delta != 0 {
            rects[idx].1.right += right_delta;
        }
        if top_delta != 0 {
            rects[idx].1.top += top_delta;
        }
        if bottom_delta != 0 {
            rects[idx].1.bottom += bottom_delta;
        }
    }
    
    // NOTE: We DON'T clear monitor_state here!
    // Resize intents must persist across multiple retile operations so that
    // window focus changes, layout updates, etc. continue to respect user's
    // manual resize preferences. The state is automatically cleaned up when:
    // - Windows are removed (via monitor_state.retain() above)
    // - User manually resizes again (update_resize_state_for_window overwrites)
    // - Addon reloads (state naturally resets)
}
