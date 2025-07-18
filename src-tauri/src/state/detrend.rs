use nalgebra::{DMatrix, DVector};

pub fn remove_linear_trend(y: &[f64]) -> Vec<f64> {
    let n = y.len();
    if n == 0 {
        return vec![];
    }

    // Create the time vector (0, 1, 2, ...)
    let t: Vec<f64> = (0..n).map(|i| i as f64).collect();

    // Set up the least-squares problem Ax = b to find coefficients for y = mt + c
    // A is the design matrix, b is the observation vector (our signal)
    let a = DMatrix::from_fn(n, 2, |r, c| if c == 0 { 1.0 } else { t[r] });
    let b = DVector::from_vec(y.to_vec());

    // Solve for the coefficients [c, m]
    let coeffs = a.svd(true, true).solve(&b, 1e-10).unwrap();
    let c = coeffs[0]; // Intercept
    let m = coeffs[1]; // Slope

    // Subtract the trend from the original signal
    y.iter()
        .zip(t.iter())
        .map(|(yi, ti)| yi - (m * ti + c))
        .collect()
}

/// Removes the best-fit quadratic trend from a signal.
pub fn remove_quadratic_trend(y: &[f64]) -> Vec<f64> {
    let n = y.len();
    if n < 3 { // Not enough points for a quadratic fit
        return remove_linear_trend(y);
    }

    let t: Vec<f64> = (0..n).map(|i| i as f64).collect();

    // Set up Ax = b for y = at^2 + bt + c
    let a = DMatrix::from_fn(n, 3, |r, c| match c {
        0 => 1.0,       // c
        1 => t[r],      // b
        _ => t[r].powi(2), // a
    });
    let b = DVector::from_vec(y.to_vec());

    // Solve for the coefficients [c, b, a]
    let coeffs = a.svd(true, true).solve(&b, 1e-10).unwrap();
    let c = coeffs[0];
    let b_coeff = coeffs[1];
    let a_coeff = coeffs[2];

    // Subtract the trend
    y.iter()
        .zip(t.iter())
        .map(|(yi, ti)| yi - (a_coeff * ti.powi(2) + b_coeff * ti + c))
        .collect()
}