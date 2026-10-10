//! Temperature sensors via `/sys/class/hwmon`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempSensor {
    pub chip: String,
    pub label: String,
    pub temp_c: f64,
}

/// Read every `temp*_input` under `/sys/class/hwmon`. Values are millidegrees.
pub fn read_temps() -> Vec<TempSensor> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir("/sys/class/hwmon") else {
        return out;
    };
    for ent in rd.flatten() {
        let base = ent.path();
        let chip = std::fs::read_to_string(base.join("name"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| ent.file_name().to_string_lossy().into_owned());
        let Ok(files) = std::fs::read_dir(&base) else {
            continue;
        };
        for f in files.flatten() {
            let fname = f.file_name().to_string_lossy().into_owned();
            let Some(stem) = fname.strip_suffix("_input") else {
                continue;
            };
            if !stem.starts_with("temp") {
                continue;
            }
            let label = std::fs::read_to_string(base.join(format!("{stem}_label")))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| stem.to_string());
            if let Ok(raw) = std::fs::read_to_string(f.path()) {
                if let Ok(milli) = raw.trim().parse::<i64>() {
                    out.push(TempSensor {
                        chip: chip.clone(),
                        label,
                        temp_c: milli as f64 / 1000.0,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| a.chip.cmp(&b.chip).then(a.label.cmp(&b.label)));
    out
}

/// Best-effort CPU package temperature.
pub fn cpu_temp_c() -> Option<f64> {
    let temps = read_temps();
    temps
        .iter()
        .find(|s| s.chip == "k10temp" || s.chip == "coretemp")
        .map(|s| s.temp_c)
        .or_else(|| {
            temps
                .iter()
                .find(|s| s.chip.contains("cpu"))
                .map(|s| s.temp_c)
        })
}

/// Best-effort GPU temperature.
pub fn gpu_temp_c() -> Option<f64> {
    read_temps()
        .iter()
        .find(|s| s.chip == "amdgpu" || s.chip == "nouveau" || s.chip.starts_with("nvidia"))
        .map(|s| s.temp_c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_temps_does_not_panic() {
        // On any real box this returns some sensors; on a bare container it returns none.
        let t = read_temps();
        for s in t {
            assert!(s.temp_c.is_finite());
        }
    }
}
