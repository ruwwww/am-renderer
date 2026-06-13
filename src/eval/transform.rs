//! Transform utilities for building affine transformation matrices.
//!
//! Layers in Alight Motion are positioned relative to the canvas center.
//! This module builds 3x3 affine transform matrices from layer properties
//! for use in compositing.

/// Build a 3x3 affine transform matrix from layer properties.
///
/// The transformation order is: translate to position, then rotate, then scale.
/// The layer's location is relative to the canvas, with the layer's center
/// at the specified position.
///
/// The matrix is column-major: `M[col][row]`.
///
/// # Arguments
/// * `location` - Layer position [x, y, z] (z is depth, used for ordering only)
/// * `scale` - Scale factors [sx, sy] where 1.0 = 100%
/// * `rotation_deg` - Rotation in degrees
/// * `canvas_center` - Center of the canvas [cx, cy]
///
/// # Returns
/// A 3x3 affine transform matrix as `[[f32; 3]; 3]` in column-major order.
pub fn build_transform_matrix(
    location: [f32; 3],
    scale: [f32; 2],
    rotation_deg: f32,
    _canvas_center: [f32; 2],
) -> [[f32; 3]; 3] {
    let angle = rotation_deg.to_radians();
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    let sx = scale[0];
    let sy = scale[1];

    // The layer position in canvas coordinates (absolute coordinates from XML)
    let tx = location[0];
    let ty = location[1];

    // Combined matrix: Translate * Rotate * Scale
    // Column-major: M[col][row]
    //
    // | sx*cos  -sy*sin  tx |
    // | sx*sin   sy*cos  ty |
    // |   0        0      1 |
    [
        [sx * cos_a, sx * sin_a, 0.0],  // column 0
        [-sy * sin_a, sy * cos_a, 0.0], // column 1
        [tx, ty, 1.0],                  // column 2
    ]
}

/// Invert a 3x3 affine transform matrix.
///
/// Used to map canvas pixel coordinates back to layer-local coordinates
/// during compositing (inverse sampling).
///
/// # Returns
/// The inverted matrix, or `None` if the matrix is singular (non-invertible).
pub fn invert_transform(m: &[[f32; 3]; 3]) -> Option<[[f32; 3]; 3]> {
    // For a 2D affine matrix stored column-major:
    // | m[0][0]  m[1][0]  m[2][0] |
    // | m[0][1]  m[1][1]  m[2][1] |
    // |   0        0        1     |
    let a = m[0][0];
    let b = m[1][0];
    let c = m[0][1];
    let d = m[1][1];
    let tx = m[2][0];
    let ty = m[2][1];

    let det = a * d - b * c;
    if det.abs() < 1e-10 {
        return None;
    }

    let inv_det = 1.0 / det;

    Some([
        [d * inv_det, -c * inv_det, 0.0], // column 0
        [-b * inv_det, a * inv_det, 0.0], // column 1
        [
            (b * ty - d * tx) * inv_det,
            (c * tx - a * ty) * inv_det,
            1.0,
        ], // column 2
    ])
}

/// Apply a 3x3 affine transform to a 2D point.
///
/// # Arguments
/// * `m` - The transform matrix (column-major)
/// * `point` - The 2D point [x, y]
///
/// # Returns
/// The transformed point [x', y'].
pub fn transform_point(m: &[[f32; 3]; 3], point: [f32; 2]) -> [f32; 2] {
    [
        m[0][0] * point[0] + m[1][0] * point[1] + m[2][0],
        m[0][1] * point[0] + m[1][1] * point[1] + m[2][1],
    ]
}
