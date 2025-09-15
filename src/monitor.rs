use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::Duration;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LhmNode {
    #[serde(rename = "Text")]
    pub text: Option<String>,
    #[serde(rename = "Value")]
    pub value: Option<String>,
    #[serde(rename = "Min")]
    pub min: Option<String>,
    #[serde(rename = "Max")]
    pub max: Option<String>,
    #[serde(rename = "Children")]
    pub children: Option<Vec<LhmNode>>,
}

#[derive(Debug, Deserialize)]
pub struct LhmResponse {
    #[serde(rename = "Children")]
    pub children: Vec<LhmNode>,
}

#[derive(Debug, Clone)]
pub struct GpuTempReading {
    pub sensor_name: String,
    pub temperature: f32,
    pub min_temp: Option<f32>,
    pub max_temp: Option<f32>,
}

pub struct TempMonitor {
    client: Client,
    api_url: String,
    gpu_patterns: Vec<String>,
}

impl TempMonitor {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_url: "http://127.0.0.1:8085/data.json".to_string(),
            gpu_patterns: vec![
                "*GPU*Core*".to_string(),
                "*GPU*Hot*".to_string(),
                "*GPU*Junction*".to_string(),
                "*Graphics*Core*".to_string(),
            ],
        }
    }

    pub async fn get_gpu_temperatures(&self) -> Result<Vec<GpuTempReading>, Box<dyn Error>> {
        let response = self.client
            .get(&self.api_url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        let lhm_data: LhmResponse = response.json().await?;
        let mut gpu_temps = Vec::new();

        self.collect_gpu_temps(&lhm_data.children, &mut gpu_temps);

        Ok(gpu_temps)
    }

    fn collect_gpu_temps(&self, nodes: &[LhmNode], gpu_temps: &mut Vec<GpuTempReading>) {
        for node in nodes {
            if let Some(text) = &node.text {
                // Check if this is a GPU temperature sensor
                if self.is_gpu_temp_sensor(text) {
                    if let Some(value) = &node.value {
                        if let Ok(temp) = self.parse_temperature(value) {
                            let reading = GpuTempReading {
                                sensor_name: text.clone(),
                                temperature: temp,
                                min_temp: node.min.as_ref().and_then(|m| self.parse_temperature(m).ok()),
                                max_temp: node.max.as_ref().and_then(|m| self.parse_temperature(m).ok()),
                            };
                            gpu_temps.push(reading);
                        }
                    }
                }
            }

            // Recursively search children
            if let Some(children) = &node.children {
                self.collect_gpu_temps(children, gpu_temps);
            }
        }
    }

    fn is_gpu_temp_sensor(&self, sensor_name: &str) -> bool {
        let name_upper = sensor_name.to_uppercase();

        // Check for GPU and temperature indicators
        let has_gpu = name_upper.contains("GPU") || name_upper.contains("GRAPHICS");
        let has_temp = name_upper.contains("CORE") ||
                      name_upper.contains("HOT") ||
                      name_upper.contains("JUNCTION") ||
                      name_upper.contains("TEMP");

        has_gpu && has_temp
    }

    fn parse_temperature(&self, temp_str: &str) -> Result<f32, Box<dyn Error>> {
        // Remove °C and other non-numeric characters, handle different locales
        let cleaned = temp_str
            .replace("°C", "")
            .replace("°F", "")
            .replace(",", ".") // Handle European decimal separator
            .trim()
            .to_string();

        // Extract just the numeric part
        let numeric_part: String = cleaned
            .chars()
            .take_while(|c| c.is_numeric() || *c == '.')
            .collect();

        if numeric_part.is_empty() {
            return Err("No numeric temperature found".into());
        }

        let temp: f32 = numeric_part.parse()?;

        // Convert Fahrenheit to Celsius if needed
        if temp_str.contains("°F") {
            Ok((temp - 32.0) * 5.0 / 9.0)
        } else {
            Ok(temp)
        }
    }

    pub async fn test_connection(&self) -> Result<(), Box<dyn Error>> {
        let response = self.client
            .get(&self.api_url)
            .send()
            .await?;

        if response.status().is_success() {
            println!("✅ LibreHardwareMonitor connection successful");
            Ok(())
        } else {
            Err(format!("LibreHardwareMonitor not available: {}", response.status()).into())
        }
    }
}