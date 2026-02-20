//! Lightweight probabilistic matmul spot-check verification.
//!
//! Instead of re-running the full matrix multiplication that a Scout computed,
//! the Verifier selects a small random subset of output rows (default 5%) and
//! re-computes only those rows. If any row deviates beyond the tolerance
//! threshold, the result is rejected and the Scout is penalised.
//!
//! This gives O(sample_rate × N) verification cost instead of O(N²).

use serde::{Deserialize, Serialize};

/// Result of a spot-check verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotCheckResult {
    pub passed: bool,
    pub rows_checked: usize,
    pub total_rows: usize,
    pub max_deviation: f32,
    pub tolerance: f32,
    pub failed_rows: Vec<usize>,
}

/// Configuration for spot-check verification.
#[derive(Debug, Clone)]
pub struct SpotCheckConfig {
    /// Fraction of rows to verify (0.0 to 1.0). Default: 0.05 (5%).
    pub sample_rate: f32,
    /// Maximum acceptable deviation per element. Default: 1e-3 for fp16.
    pub tolerance: f32,
    /// Minimum number of rows to always check regardless of sample rate.
    pub min_rows: usize,
}

impl Default for SpotCheckConfig {
    fn default() -> Self {
        Self {
            sample_rate: 0.05,
            tolerance: 1e-3,
            min_rows: 1,
        }
    }
}

/// Select which row indices to verify using a deterministic seed.
pub fn select_check_rows(total_rows: usize, config: &SpotCheckConfig, seed: u64) -> Vec<usize> {
    if total_rows == 0 {
        return vec![];
    }

    let n_rows = ((total_rows as f32 * config.sample_rate).ceil() as usize).max(config.min_rows);
    let n_rows = n_rows.min(total_rows);

    // Simple deterministic selection using seed (LCG-based)
    let mut indices = Vec::with_capacity(n_rows);
    let mut rng_state = seed;
    let mut selected = std::collections::HashSet::new();

    while indices.len() < n_rows {
        // Linear congruential generator step
        rng_state = rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let idx = (rng_state >> 33) as usize % total_rows;
        if selected.insert(idx) {
            indices.push(idx);
        }
    }

    indices.sort();
    indices
}

/// Perform spot-check verification of a matrix multiplication result.
///
/// Given input matrix A (M×K), weight matrix B (K×N), and claimed output C (M×N),
/// verify random rows of C by recomputing `C[row] = A[row] × B`.
///
/// All matrices are in row-major f32 format.
pub fn verify_matmul(
    input_a: &[f32],   // M×K flattened
    weights_b: &[f32], // K×N flattened
    claimed_c: &[f32], // M×N flattened
    m: usize,          // rows of A / rows of C
    k: usize,          // cols of A / rows of B
    n: usize,          // cols of B / cols of C
    config: &SpotCheckConfig,
    seed: u64,
) -> SpotCheckResult {
    // Validate dimensions
    if input_a.len() != m * k || weights_b.len() != k * n || claimed_c.len() != m * n {
        return SpotCheckResult {
            passed: false,
            rows_checked: 0,
            total_rows: m,
            max_deviation: f32::INFINITY,
            tolerance: config.tolerance,
            failed_rows: vec![],
        };
    }

    let check_rows = select_check_rows(m, config, seed);
    let mut max_deviation: f32 = 0.0;
    let mut failed_rows = Vec::new();

    for &row in &check_rows {
        for col in 0..n {
            // Recompute C[row][col] = sum(A[row][i] * B[i][col] for i in 0..k)
            let mut expected: f32 = 0.0;
            for i in 0..k {
                expected += input_a[row * k + i] * weights_b[i * n + col];
            }

            let claimed = claimed_c[row * n + col];
            let deviation = (expected - claimed).abs();
            max_deviation = max_deviation.max(deviation);

            if deviation > config.tolerance && !failed_rows.contains(&row) {
                failed_rows.push(row);
            }
        }
    }

    SpotCheckResult {
        passed: failed_rows.is_empty(),
        rows_checked: check_rows.len(),
        total_rows: m,
        max_deviation,
        tolerance: config.tolerance,
        failed_rows,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_matmul(a: &[f32], b: &[f32], m: usize, k: usize, n: usize) -> Vec<f32> {
        let mut c = vec![0.0f32; m * n];
        for row in 0..m {
            for col in 0..n {
                for i in 0..k {
                    c[row * n + col] += a[row * k + i] * b[i * n + col];
                }
            }
        }
        c
    }

    #[test]
    fn correct_matmul_passes() {
        let m = 8;
        let k = 4;
        let n = 6;
        let a: Vec<f32> = (0..m * k).map(|i| (i as f32) * 0.1).collect();
        let b: Vec<f32> = (0..k * n).map(|i| (i as f32) * 0.05).collect();
        let c = simple_matmul(&a, &b, m, k, n);

        let config = SpotCheckConfig {
            sample_rate: 1.0, // check all rows
            ..Default::default()
        };

        let result = verify_matmul(&a, &b, &c, m, k, n, &config, 42);
        assert!(result.passed);
        assert_eq!(result.rows_checked, m);
        assert!(result.max_deviation < 1e-5);
        assert!(result.failed_rows.is_empty());
    }

    #[test]
    fn tampered_output_fails() {
        let m = 8;
        let k = 4;
        let n = 6;
        let a: Vec<f32> = (0..m * k).map(|i| (i as f32) * 0.1).collect();
        let b: Vec<f32> = (0..k * n).map(|i| (i as f32) * 0.05).collect();
        let mut c = simple_matmul(&a, &b, m, k, n);

        // Tamper with row 3
        c[3 * n] += 10.0;
        c[3 * n + 1] += 10.0;

        let config = SpotCheckConfig {
            sample_rate: 1.0, // check all rows to guarantee catching it
            ..Default::default()
        };

        let result = verify_matmul(&a, &b, &c, m, k, n, &config, 42);
        assert!(!result.passed);
        assert!(result.failed_rows.contains(&3));
        assert!(result.max_deviation > 9.0);
    }

    #[test]
    fn tolerance_boundary() {
        let m = 4;
        let k = 2;
        let n = 3;
        let a: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
        let mut c = simple_matmul(&a, &b, m, k, n);

        // Add tiny perturbation within tolerance
        c[0] += 5e-4;

        let config = SpotCheckConfig {
            sample_rate: 1.0,
            tolerance: 1e-3,
            min_rows: 1,
        };

        let result = verify_matmul(&a, &b, &c, m, k, n, &config, 42);
        assert!(result.passed, "perturbation within tolerance should pass");

        // Add perturbation exceeding tolerance
        c[0] += 2e-3;
        let result2 = verify_matmul(&a, &b, &c, m, k, n, &config, 42);
        assert!(
            !result2.passed,
            "perturbation exceeding tolerance should fail"
        );
    }

    #[test]
    fn select_rows_deterministic() {
        let rows1 = select_check_rows(100, &SpotCheckConfig::default(), 42);
        let rows2 = select_check_rows(100, &SpotCheckConfig::default(), 42);
        assert_eq!(rows1, rows2, "same seed should give same rows");

        let rows3 = select_check_rows(100, &SpotCheckConfig::default(), 99);
        assert_ne!(
            rows1, rows3,
            "different seeds should likely give different rows"
        );
    }

    #[test]
    fn empty_matrix() {
        let result = verify_matmul(&[], &[], &[], 0, 0, 0, &SpotCheckConfig::default(), 42);
        assert!(result.passed);
        assert_eq!(result.rows_checked, 0);
    }

    #[test]
    fn dimension_mismatch_fails() {
        let a = vec![1.0f32; 6]; // 2×3
        let b = vec![1.0f32; 6]; // 3×2
        let c = vec![1.0f32; 3]; // wrong: should be 2×2 = 4

        let result = verify_matmul(&a, &b, &c, 2, 3, 2, &SpotCheckConfig::default(), 42);
        assert!(!result.passed);
        assert_eq!(result.max_deviation, f32::INFINITY);
    }

    #[test]
    fn min_rows_enforced() {
        let config = SpotCheckConfig {
            sample_rate: 0.0, // would select 0 rows
            min_rows: 3,
            ..Default::default()
        };
        let rows = select_check_rows(100, &config, 42);
        assert_eq!(rows.len(), 3);
    }
}
