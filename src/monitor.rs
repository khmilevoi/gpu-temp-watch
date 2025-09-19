use nvml_wrapper::Nvml;
use std::error::Error;
use crate::{log_info, log_error, log_warn};

#[derive(Debug, Clone)]
pub struct GpuTempReading {
    pub sensor_name: String,
    pub temperature: f32,
    pub min_temp: Option<f32>,
    pub max_temp: Option<f32>,
}

pub struct TempMonitor {
    nvml: Option<Nvml>,
}

impl TempMonitor {
    pub fn new() -> Self {
        match Nvml::init() {
            Ok(nvml) => {
                log_info!("NVML initialized successfully");
                Self { nvml: Some(nvml) }
            }
            Err(e) => {
                log_error!("Failed to initialize NVML", serde_json::json!({
                    "error": format!("{}", e),
                    "warning": "GPU temperature monitoring will not be available"
                }));
                Self { nvml: None }
            }
        }
    }

    pub async fn get_gpu_temperatures(&self) -> Result<Vec<GpuTempReading>, Box<dyn Error>> {
        let nvml = match &self.nvml {
            Some(nvml) => nvml,
            None => {
                log_error!("NVML not initialized - cannot read GPU temperatures");
                return Err("NVML not initialized".into());
            }
        };

        let mut gpu_temps = Vec::new();
        let mut errors = Vec::new();

        let device_count = match nvml.device_count() {
            Ok(count) => {
                count
            }
            Err(e) => {
                log_error!("Failed to get GPU device count", serde_json::json!({"error": format!("{}", e)}));
                return Err(format!("Failed to get GPU device count: {}", e).into());
            }
        };

        if device_count == 0 {
            log_warn!("No GPU devices found");
            return Ok(gpu_temps);
        }

        for device_index in 0..device_count {
            match nvml.device_by_index(device_index) {
                Ok(device) => {
                    let name = device
                        .name()
                        .unwrap_or_else(|_| format!("GPU {}", device_index));


                    match device
                        .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
                    {
                        Ok(temp) => {
                            if temp > 0 && temp < 200 {  // Sanity check for reasonable temperature range

                                let reading = GpuTempReading {
                                    sensor_name: name,
                                    temperature: temp as f32,
                                    min_temp: None, // NVML doesn't provide min/max history by default
                                    max_temp: None,
                                };
                                gpu_temps.push(reading);
                            } else {
                                log_warn!("Invalid temperature reading (out of range)", serde_json::json!({
                                    "sensor": name,
                                    "temperature": temp,
                                    "valid_range": "0-200°C"
                                }));
                                errors.push(format!("Invalid temperature for {}: {}°C", name, temp));
                            }
                        }
                        Err(e) => {
                            log_error!("Failed to read temperature", serde_json::json!({
                                "sensor": name,
                                "error": format!("{}", e)
                            }));
                            errors.push(format!("Failed to read temperature for {}: {}", name, e));
                        }
                    }
                }
                Err(e) => {
                    log_error!("Failed to access GPU device", serde_json::json!({
                        "device_index": device_index,
                        "error": format!("{}", e)
                    }));
                    errors.push(format!("Failed to access GPU device {}: {}", device_index, e));
                }
            }
        }

        if gpu_temps.is_empty() {
            let error_msg = if errors.is_empty() {
                "No GPU temperature readings available".to_string()
            } else {
                format!("No GPU temperature readings available. Errors: {}", errors.join("; "))
            };
            log_warn!("No GPU temperature readings available", serde_json::json!({
                "errors": errors,
                "action": "continuing_monitoring"
            }));
        } else {
            log_info!("Successfully read GPU temperatures", serde_json::json!({
                "count": gpu_temps.len(),
                "sensors": gpu_temps.iter().map(|r| &r.sensor_name).collect::<Vec<_>>()
            }));
        }

        Ok(gpu_temps)
    }

    pub async fn test_connection(&self) -> Result<(), Box<dyn Error>> {
        match &self.nvml {
            Some(nvml) => {
                let device_count = nvml.device_count()?;
                log_info!("NVML connection successful", serde_json::json!({
                    "gpu_devices_found": device_count
                }));
                Ok(())
            }
            None => Err("NVML not initialized".into()),
        }
    }
}
