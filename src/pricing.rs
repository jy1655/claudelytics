use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone, serde::Serialize)]
pub struct ModelPricing {
    #[serde(rename = "input_cost_per_token")]
    pub input_cost_per_token: Option<f64>,
    #[serde(rename = "output_cost_per_token")]
    pub output_cost_per_token: Option<f64>,
    #[serde(rename = "cache_creation_input_token_cost")]
    pub cache_creation_input_token_cost: Option<f64>,
    #[serde(rename = "cache_read_input_token_cost")]
    pub cache_read_input_token_cost: Option<f64>,
}

pub struct PricingFetcher;

impl Default for PricingFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PricingFetcher {
    pub fn new() -> Self {
        Self
    }

    pub fn get_model_pricing(
        &self,
        pricing_data: &HashMap<String, ModelPricing>,
        model_name: &str,
    ) -> Option<ModelPricing> {
        // Direct match first
        if let Some(pricing) = pricing_data.get(model_name) {
            return Some(pricing.clone());
        }

        // Try common name variations for Claude models
        let claude_variations = [
            model_name.to_string(),
            format!("claude-3-5-{}", model_name.trim_start_matches("claude-")),
            format!("claude-3-{}", model_name.trim_start_matches("claude-")),
            format!("claude-{}", model_name.trim_start_matches("claude-")),
            model_name.replace("claude-", ""),
        ];

        for variation in &claude_variations {
            if !variation.is_empty()
                && let Some(pricing) = pricing_data.get(variation)
            {
                return Some(pricing.clone());
            }
        }

        // Partial match as fallback
        for (key, pricing) in pricing_data {
            if key.contains(model_name) || model_name.contains(key) {
                return Some(pricing.clone());
            }
        }

        None
    }

    pub fn calculate_cost(
        &self,
        pricing: &ModelPricing,
        input_tokens: u64,
        output_tokens: u64,
        cache_creation_tokens: u64,
        cache_read_tokens: u64,
    ) -> f64 {
        let mut total_cost = 0.0;

        if let Some(input_cost) = pricing.input_cost_per_token {
            total_cost += input_tokens as f64 * input_cost;
        }

        if let Some(output_cost) = pricing.output_cost_per_token {
            total_cost += output_tokens as f64 * output_cost;
        }

        if let Some(cache_creation_cost) = pricing.cache_creation_input_token_cost {
            total_cost += cache_creation_tokens as f64 * cache_creation_cost;
        }

        if let Some(cache_read_cost) = pricing.cache_read_input_token_cost {
            total_cost += cache_read_tokens as f64 * cache_read_cost;
        }

        total_cost
    }
}

// Fallback pricing for common Claude models (updated from official pricing pages).
// Cache creation/read values use the 5-minute prompt caching rates.
pub fn get_fallback_pricing() -> HashMap<String, ModelPricing> {
    let mut pricing = HashMap::new();

    let mut insert_model =
        |keys: &[&str], input: f64, output: f64, cache_write: f64, cache_read: f64| {
            let model_pricing = ModelPricing {
                input_cost_per_token: Some(input / 1_000_000.0),
                output_cost_per_token: Some(output / 1_000_000.0),
                cache_creation_input_token_cost: Some(cache_write / 1_000_000.0),
                cache_read_input_token_cost: Some(cache_read / 1_000_000.0),
            };

            for key in keys {
                pricing.insert((*key).to_string(), model_pricing.clone());
            }
        };

    // Opus family
    insert_model(
        &["claude-opus-4-6", "claude-opus-4-6-latest"],
        5.0,
        25.0,
        6.0,
        0.5,
    );
    insert_model(
        &[
            "claude-opus-4-5-20251101",
            "claude-opus-4-5",
            "claude-opus-4-5-latest",
        ],
        5.0,
        25.0,
        6.0,
        0.5,
    );
    insert_model(
        &[
            "claude-opus-4-1-20250805",
            "claude-opus-4-1",
            "claude-opus-4-1-latest",
        ],
        15.0,
        75.0,
        18.75,
        1.5,
    );
    insert_model(
        &[
            "claude-opus-4-20250514",
            "claude-opus-4",
            "claude-opus-4-latest",
        ],
        15.0,
        75.0,
        18.75,
        1.5,
    );
    insert_model(
        &["claude-3-opus-20240229", "claude-3-opus"],
        15.0,
        75.0,
        18.75,
        1.5,
    );

    // Sonnet family
    insert_model(
        &["claude-sonnet-4-6", "claude-sonnet-4-6-latest"],
        3.0,
        15.0,
        3.75,
        0.3,
    );
    insert_model(
        &[
            "claude-sonnet-4-5-20250929",
            "claude-sonnet-4-5",
            "claude-sonnet-4-5-latest",
        ],
        3.0,
        15.0,
        3.75,
        0.3,
    );
    insert_model(
        &[
            "claude-sonnet-4-20250514",
            "claude-sonnet-4",
            "claude-sonnet-4-latest",
        ],
        3.0,
        15.0,
        3.75,
        0.3,
    );
    insert_model(
        &[
            "claude-3-7-sonnet-20250219",
            "claude-3-7-sonnet",
            "claude-3-7-sonnet-latest",
        ],
        3.0,
        15.0,
        3.75,
        0.3,
    );
    insert_model(
        &[
            "claude-3-5-sonnet-20241022",
            "claude-3-5-sonnet",
            "claude-3-5-sonnet-latest",
        ],
        3.0,
        15.0,
        3.75,
        0.3,
    );
    insert_model(
        &["claude-3-sonnet-20240229", "claude-3-sonnet"],
        3.0,
        15.0,
        3.75,
        0.3,
    );

    // Haiku family
    insert_model(
        &[
            "claude-haiku-4-5-20251001",
            "claude-haiku-4-5",
            "claude-haiku-4-5-latest",
        ],
        1.0,
        5.0,
        1.25,
        0.1,
    );
    insert_model(
        &[
            "claude-3-5-haiku-20241022",
            "claude-3-5-haiku",
            "claude-3-5-haiku-latest",
        ],
        0.8,
        4.0,
        1.0,
        0.08,
    );
    insert_model(
        &["claude-3-haiku-20240307", "claude-3-haiku"],
        0.25,
        1.25,
        0.3,
        0.03,
    );

    pricing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opus_4_5_uses_updated_pricing() {
        let fetcher = PricingFetcher::new();
        let pricing_data = get_fallback_pricing();

        let pricing = fetcher
            .get_model_pricing(&pricing_data, "claude-opus-4-5-20251101")
            .expect("pricing should exist for opus 4.5");

        let input_cost = pricing
            .input_cost_per_token
            .expect("input cost should be present");
        let output_cost = pricing
            .output_cost_per_token
            .expect("output cost should be present");

        assert!((input_cost - (5.0 / 1_000_000.0)).abs() < f64::EPSILON);
        assert!((output_cost - (25.0 / 1_000_000.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_haiku_3_cache_write_uses_official_rate() {
        let fetcher = PricingFetcher::new();
        let pricing_data = get_fallback_pricing();

        let pricing = fetcher
            .get_model_pricing(&pricing_data, "claude-3-haiku-20240307")
            .expect("pricing should exist for haiku 3");

        let cache_write_cost = pricing
            .cache_creation_input_token_cost
            .expect("cache write cost should be present");

        assert!((cache_write_cost - (0.3 / 1_000_000.0)).abs() < f64::EPSILON);
    }
}
