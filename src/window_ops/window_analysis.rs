/// Window alignment and neighbor analysis
/// Detects adjacent and aligned windows for resize operations

use windows::Win32::Foundation::{HWND, RECT};
use super::ops_utils::EDGE_TOLERANCE;

/// Find windows adjacent to a given window on a specific side
/// Used for propagating resize operations
pub fn find_neighbor_indices(
    rects: &[(HWND, RECT)],
    idx: usize,
    side: &str,
    max_gap: i32,
) -> Vec<usize> {
    let Some(rect) = rects.get(idx).map(|r| r.1) else {
        return Vec::new();
    };
    let mut neighbors = Vec::new();

    let boundary = match side {
        "right" => rect.right,
        "left" => rect.left,
        "top" => rect.top,
        "bottom" => rect.bottom,
        _ => return neighbors,
    };

    for (j, (_, other)) in rects.iter().enumerate() {
        if j == idx {
            continue;
        }

        let aligned = match side {
            "right" => (other.left - boundary).abs() <= max_gap + EDGE_TOLERANCE,
            "left" => (other.right - boundary).abs() <= max_gap + EDGE_TOLERANCE,
            "top" => (other.bottom - boundary).abs() <= max_gap + EDGE_TOLERANCE,
            "bottom" => (other.top - boundary).abs() <= max_gap + EDGE_TOLERANCE,
            _ => false,
        };

        if aligned {
            neighbors.push(j);
        }
    }

    neighbors
}

/// Find windows with edges aligned to the same side
/// Used for keeping aligned edges during resize
pub fn find_aligned_indices(
    rects: &[(HWND, RECT)],
    idx: usize,
    side: &str,
) -> Vec<usize> {
    let Some(rect) = rects.get(idx).map(|r| r.1) else {
        return Vec::new();
    };
    let mut aligned = Vec::new();

    let boundary = match side {
        "right" => rect.right,
        "left" => rect.left,
        "top" => rect.top,
        "bottom" => rect.bottom,
        _ => return aligned,
    };

    let tol = EDGE_TOLERANCE * 4;
    for (j, (_, other)) in rects.iter().enumerate() {
        let matches = match side {
            "right" => (other.right - boundary).abs() <= tol,
            "left" => (other.left - boundary).abs() <= tol,
            "top" => (other.top - boundary).abs() <= tol,
            "bottom" => (other.bottom - boundary).abs() <= tol,
            _ => false,
        };

        if matches {
            aligned.push(j);
        }
    }

    aligned
}
