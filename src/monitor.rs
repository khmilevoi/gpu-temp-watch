use nvml_wrapper::Nvml;
use std::error::Error;
use tracing::{debug, error, info, warn};

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
                info!("✅ NVML initialized successfully");
                Self { nvml: Some(nvml) }
            }
            Err(e) => {
                error!("❌ Failed to initialize NVML: {}", e);
                warn!("GPU temperature monitoring will not be available");
                Self { nvml: None }
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_gpu_temperatures(&self) -> Result<Vec<GpuTempReading>, Box<dyn Error>> {
        let nvml = match &self.nvml {
            Some(nvml) => nvml,
            None => {
                error!("❌ NVML not initialized - cannot read GPU temperatures");
                return Err("NVML not initialized".into());
            }
        };

        let mut gpu_temps = Vec::new();
        let mut errors = Vec::new();

        let device_count = match nvml.device_count() {
            Ok(count) => {
                debug!("Found {} GPU devices", count);
                count
            }
            Err(e) => {
                error!("❌ Failed to get GPU device count: {}", e);
                return Err(format!("Failed to get GPU device count: {}", e).into());
            }
        };

        if device_count == 0 {
            warn!("⚠️  No GPU devices found");
            return Ok(gpu_temps);
        }

        for device_index in 0..device_count {
            match nvml.device_by_index(device_index) {
                Ok(device) => {
                    let name = device
                        .name()
                        .unwrap_or_else(|_| format!("GPU {}", device_index));

                    debug!("🔍 Attempting to read temperature for {}", name);

                    match device
                        .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
                    {
                        Ok(temp) => {
                            if temp > 0 && temp < 200 {  // Sanity check for reasonable temperature range
                                debug!("📊 {}: {}°C", name, temp);

                                let reading = GpuTempReading {
                                    sensor_name: name,
                                    temperature: temp as f32,
                                    min_temp: None, // NVML doesn't provide min/max history by default
                                    max_temp: None,
                                };
                                gpu_temps.push(reading);
                            } else {
                                warn!("⚠️  Invalid temperature reading for {}: {}°C (out of range)", name, temp);
                                errors.push(format!("Invalid temperature for {}: {}°C", name, temp));
                            }
                        }
                        Err(e) => {
                            warn!("⚠️  Failed to read temperature for {}: {}", name, e);
                            errors.push(format!("Failed to read temperature for {}: {}", name, e));
                        }
                    }
                }
                Err(e) => {
                    warn!("⚠️  Failed to access GPU device {}: {}", device_index, e);
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
            warn!("⚠️  {}", error_msg);
            
            // Don't return an error if we simply have no readings, just log it
            // This allows the application to continue running
            info!("📝 Continuing monitoring despite no temperature readings");
        } else {
            info!("✅ Successfully read {} GPU temperature(s)", gpu_temps.len());
        }

        Ok(gpu_temps)
    }

    #[tracing::instrument(skip(self))]
    pub async fn test_connection(&self) -> Result<(), Box<dyn Error>> {
        match &self.nvml {
            Some(nvml) => {
                let device_count = nvml.device_count()?;
                info!(
                    "✅ NVML connection successful - {} GPU devices found",
                    device_count
                );
                Ok(())
            }
            None => Err("NVML not initialized".into()),
        }
    }
}
