use serde::{Deserialize, Serialize};

/// Must mirror the TypeScript `WorkoutType` in src/lib/metrics.ts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkoutType {
    Recovery,
    Endurance,
    Tempo,
    #[serde(rename = "Sweet Spot")]
    SweetSpot,
    Threshold,
    #[serde(rename = "VO2max")]
    VO2max,
    Anaerobic,
}

impl WorkoutType {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkoutType::Recovery => "Recovery",
            WorkoutType::Endurance => "Endurance",
            WorkoutType::Tempo => "Tempo",
            WorkoutType::SweetSpot => "Sweet Spot",
            WorkoutType::Threshold => "Threshold",
            WorkoutType::VO2max => "VO2max",
            WorkoutType::Anaerobic => "Anaerobic",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Recovery" => Some(WorkoutType::Recovery),
            "Endurance" => Some(WorkoutType::Endurance),
            "Tempo" => Some(WorkoutType::Tempo),
            "Sweet Spot" => Some(WorkoutType::SweetSpot),
            "Threshold" => Some(WorkoutType::Threshold),
            "VO2max" => Some(WorkoutType::VO2max),
            "Anaerobic" => Some(WorkoutType::Anaerobic),
            _ => None,
        }
    }
}

const ZONE_THRESHOLDS: [f32; 5] = [0.55, 0.75, 0.90, 1.05, 1.20];

fn zone_of(pct: f32) -> usize {
    for (i, t) in ZONE_THRESHOLDS.iter().enumerate() {
        if pct < *t {
            return i + 1;
        }
    }
    6
}

/// Must mirror the TypeScript `classify` in src/lib/metrics.ts.
pub fn classify(series: &[f32], if_: f32) -> WorkoutType {
    let total = series.len();
    if total == 0 || if_ < 0.55 {
        return WorkoutType::Recovery;
    }

    let mut zone_time = [0u32; 6];
    let mut ss_time = 0u32;
    for &pct in series {
        zone_time[zone_of(pct) - 1] += 1;
        if (0.83..0.95).contains(&pct) {
            ss_time += 1;
        }
    }

    let t = total as f32;
    if zone_time[5] as f32 / t > 0.05 {
        return WorkoutType::Anaerobic;
    }
    if zone_time[4] as f32 / t > 0.10 {
        return WorkoutType::VO2max;
    }
    if zone_time[3] as f32 / t > 0.15 {
        return WorkoutType::Threshold;
    }
    if ss_time as f32 / t > 0.20 {
        return WorkoutType::SweetSpot;
    }
    if zone_time[2] as f32 / t > 0.20 {
        return WorkoutType::Tempo;
    }
    if (zone_time[1] + zone_time[2]) as f32 / t > 0.40 {
        return WorkoutType::Endurance;
    }
    WorkoutType::Recovery
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_low_intensity_is_recovery() {
        let series = vec![0.4_f32; 600];
        assert_eq!(classify(&series, 0.4), WorkoutType::Recovery);
    }

    #[test]
    fn classify_vo2_burst_dominates() {
        let mut series = vec![0.6_f32; 1800];
        series.extend(vec![1.10_f32; 300]); // > 10% in zone 5
        assert_eq!(classify(&series, 0.85), WorkoutType::VO2max);
    }

    #[test]
    fn classify_sweet_spot_band() {
        let series = vec![0.88_f32; 1800];
        assert_eq!(classify(&series, 0.88), WorkoutType::SweetSpot);
    }

    #[test]
    fn workout_type_serializes_sweet_spot_with_space() {
        let s = serde_json::to_string(&WorkoutType::SweetSpot).unwrap();
        assert_eq!(s, "\"Sweet Spot\"");
    }
}
