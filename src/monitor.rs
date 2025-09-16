use log::{debug, error, info, warn};
use nvml_wrapper::Nvml;
use std::error::Error;

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

    pub async fn get_gpu_temperatures(&self) -> Result<Vec<GpuTempReading>, Box<dyn Error>> {
        let nvml = match &self.nvml {
            Some(nvml) => nvml,
            None => return Err("NVML not initialized".into()),
        };

        let mut gpu_temps = Vec::new();

        let device_count = nvml.device_count()?;
        debug!("Found {} GPU devices", device_count);

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
                            debug!("📊 {}: {}°C", name, temp);

                            let reading = GpuTempReading {
                                sensor_name: name,
                                temperature: temp as f32,
                                min_temp: None, // NVML doesn't provide min/max history by default
                                max_temp: None,
                            };
                            gpu_temps.push(reading);
                        }
                        Err(e) => {
                            warn!("⚠️  Failed to read temperature for {}: {}", name, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("⚠️  Failed to access GPU device {}: {}", device_index, e);
                }
            }
        }

        if gpu_temps.is_empty() {
            warn!("⚠️  No GPU temperature readings available");
        }

        Ok(gpu_temps)
    }

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
