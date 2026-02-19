#[derive(Debug, Clone)]
pub struct CostEstimateInput {
    pub tokens_processed_total: u64,
    pub offload_percent: f64,
    pub gpu_utilization_delta_percent: f64,
    pub cloud_gpu_usd_per_million_tokens: f64,
}

#[derive(Debug, Clone)]
pub struct CostEstimateOutput {
    pub equivalent_cloud_gpu_cost_usd: f64,
    pub estimated_savings_percent: f64,
    pub estimated_savings_usd: f64,
}

pub fn estimate(input: &CostEstimateInput) -> CostEstimateOutput {
    let tokens_million = input.tokens_processed_total as f64 / 1_000_000.0;
    let baseline_cost = tokens_million * input.cloud_gpu_usd_per_million_tokens.max(0.0);
    let savings_pct =
        (input.offload_percent * 0.8 + input.gpu_utilization_delta_percent * 0.2).clamp(0.0, 95.0);
    let savings_usd = baseline_cost * (savings_pct / 100.0);
    CostEstimateOutput {
        equivalent_cloud_gpu_cost_usd: baseline_cost,
        estimated_savings_percent: savings_pct,
        estimated_savings_usd: savings_usd,
    }
}

#[cfg(test)]
mod tests {
    use super::{estimate, CostEstimateInput};

    #[test]
    fn cost_estimate_scales_with_tokens_and_offload() {
        let out = estimate(&CostEstimateInput {
            tokens_processed_total: 2_000_000,
            offload_percent: 50.0,
            gpu_utilization_delta_percent: 30.0,
            cloud_gpu_usd_per_million_tokens: 4.0,
        });
        assert!(out.equivalent_cloud_gpu_cost_usd > 0.0);
        assert!(out.estimated_savings_percent > 0.0);
        assert!(out.estimated_savings_usd > 0.0);
    }
}
