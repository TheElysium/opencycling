use serde::{Deserialize, Serialize};
use specta::Type;

/// Bridge enum shared with the frontend via generated bindings (src/lib/bindings.ts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
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

/// Normalized Power (Coggan): the 4th root of the mean of the 4th powers of the
/// 30-second rolling average power, computed over a 1 Hz power series (watts).
/// Returns `None` for an empty series; falls back to mean power when there are
/// fewer than 30 samples (one full window).
pub fn normalized_power(power_w: &[f64]) -> Option<f32> {
    const WINDOW: usize = 30;
    if power_w.is_empty() {
        return None;
    }
    if power_w.len() < WINDOW {
        let mean = power_w.iter().sum::<f64>() / power_w.len() as f64;
        return Some(mean as f32);
    }

    // Step 1: 30 s rolling average, maintained with a running sum (O(n)).
    // Steps 2-3: accumulate each rolling average to the 4th power.
    let mut window_sum: f64 = power_w[..WINDOW].iter().sum();
    let mut sum4 = (window_sum / WINDOW as f64).powi(4);
    let mut count: u32 = 1;
    for i in WINDOW..power_w.len() {
        window_sum += power_w[i] - power_w[i - WINDOW];
        sum4 += (window_sum / WINDOW as f64).powi(4);
        count += 1;
    }

    // Step 4: 4th root of the mean.
    Some((sum4 / count as f64).powf(0.25) as f32)
}

/// Derived performance metrics computed from the full 1 Hz power series.
/// All fields are `None` together when no power was recorded or FTP is zero.
#[derive(Debug, Default, PartialEq)]
pub struct DerivedMetrics {
    pub workout_type: Option<WorkoutType>,
    /// Normalized Power in watts (Coggan).
    pub np_w: Option<f32>,
    /// Intensity Factor: NP / FTP.
    pub if_: Option<f32>,
    /// Training Stress Score: (duration_h) * IF^2 * 100.
    pub tss: Option<f32>,
}

/// Pure computation of NP, IF, TSS, and workout classification from a 1 Hz
/// power series (watts as f64), the frozen FTP (watts), and the active session
/// duration. Returns `DerivedMetrics::default()` (all `None`) when `powers` is
/// empty or `ftp_w` is zero.
pub fn derive_metrics(powers: &[f64], ftp_w: u16, duration_s: u32) -> DerivedMetrics {
    if ftp_w == 0 || powers.is_empty() {
        return DerivedMetrics::default();
    }
    let ftp = ftp_w as f32;
    let Some(np_w) = normalized_power(powers) else {
        return DerivedMetrics::default();
    };
    let if_ = np_w / ftp;
    let tss = (duration_s as f32 / 3600.0) * if_ * if_ * 100.0;
    let series_pcts: Vec<f32> = powers.iter().map(|&w| w as f32 / ftp).collect();
    DerivedMetrics {
        workout_type: Some(classify(&series_pcts, if_)),
        np_w: Some(np_w),
        if_: Some(if_),
        tss: Some(tss),
    }
}

/// Power zone upper bounds as fractions of FTP. Index i is the upper bound of
/// zone i+1; anything at or above the last bound is zone 6 (anaerobic).
const ZONE_THRESHOLDS: [f32; 5] = [0.55, 0.75, 0.90, 1.05, 1.20];

/// Power zone (1..=6) for a fraction of FTP. Single source of truth for zone
/// boundaries on the Rust side, used by `classify` and by block labeling.
pub fn zone_of(pct: f32) -> u8 {
    for (i, t) in ZONE_THRESHOLDS.iter().enumerate() {
        if pct < *t {
            return (i as u8) + 1;
        }
    }
    6
}

/// Human-readable name for a power zone (1..=6). Used for synthesized block labels.
pub fn zone_name(z: u8) -> &'static str {
    match z {
        1 => "Recovery",
        2 => "Endurance",
        3 => "Tempo",
        4 => "Threshold",
        5 => "VO2max",
        _ => "Anaerobic",
    }
}

/// Fallback label for a flattened block that carries no explicit name: describes
/// whether it is steady or a ramp and the zone(s) it spans. Matches the zone
/// boundaries above so the frontend and backend agree.
pub fn fallback_label(start_w: u16, end_w: u16, ftp_w: u16) -> String {
    if ftp_w == 0 {
        return "Block".to_string();
    }
    let ftp = ftp_w as f32;
    let zs = zone_of(start_w as f32 / ftp);
    let ze = zone_of(end_w as f32 / ftp);
    if zs != ze {
        format!("Ramp {}→{}", zone_name(zs), zone_name(ze))
    } else {
        format!("Steady {}", zone_name(zs))
    }
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
        zone_time[zone_of(pct) as usize - 1] += 1;
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
    fn zone_of_pins_boundaries() {
        // Zone upper bounds are exclusive: exactly at a threshold means the next zone.
        assert_eq!(zone_of(0.0), 1);
        assert_eq!(zone_of(0.54), 1);
        assert_eq!(zone_of(0.55), 2);
        assert_eq!(zone_of(0.75), 3);
        assert_eq!(zone_of(0.90), 4);
        assert_eq!(zone_of(1.05), 5);
        assert_eq!(zone_of(1.20), 6);
        assert_eq!(zone_of(2.0), 6);
    }

    #[test]
    fn fallback_label_steady_ramp_and_zero_ftp() {
        // 150 W at 200 W FTP = 75% -> zone 3 (Tempo); same start/end means Steady.
        assert_eq!(fallback_label(150, 150, 200), "Steady Tempo");
        // 100 W -> 50% (Recovery) up to 200 W -> 100% (Threshold): a cross-zone Ramp.
        assert_eq!(fallback_label(100, 200, 200), "Ramp Recovery→Threshold");
        // FTP 0 cannot be classified.
        assert_eq!(fallback_label(100, 200, 0), "Block");
    }

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
    fn np_constant_power_equals_that_power() {
        // Rolling 30s average of a constant series is that constant, so NP == P.
        let series = vec![200.0_f64; 60];
        assert_eq!(normalized_power(&series), Some(200.0));
    }

    #[test]
    fn np_empty_series_is_none() {
        assert_eq!(normalized_power(&[]), None);
    }

    #[test]
    fn np_short_series_falls_back_to_mean() {
        // Fewer than 30 samples: not enough for a full window, use mean power.
        assert_eq!(normalized_power(&[100.0, 200.0, 300.0]), Some(200.0));
    }

    #[test]
    fn np_exceeds_mean_for_variable_power() {
        // 60 s blocks alternating 100/300 W average 200 W, but because the surges
        // outlast the 30 s smoothing window NP weights them higher than the mean.
        let series: Vec<f64> = (0..600)
            .map(|i| if (i / 60) % 2 == 0 { 100.0 } else { 300.0 })
            .collect();
        let np = normalized_power(&series).unwrap();
        assert!(np > 200.0, "NP {np} should exceed the 200 W mean");
    }

    #[test]
    fn workout_type_serializes_sweet_spot_with_space() {
        let s = serde_json::to_string(&WorkoutType::SweetSpot).unwrap();
        assert_eq!(s, "\"Sweet Spot\"");
    }

    // --- derive_metrics tests ---

    #[test]
    fn derive_metrics_empty_powers_returns_all_none() {
        let d = derive_metrics(&[], 250, 3600);
        assert!(d.np_w.is_none());
        assert!(d.if_.is_none());
        assert!(d.tss.is_none());
        assert!(d.workout_type.is_none());
    }

    #[test]
    fn derive_metrics_ftp_zero_returns_all_none() {
        let powers = vec![200.0_f64; 60];
        let d = derive_metrics(&powers, 0, 3600);
        assert!(d.np_w.is_none());
        assert!(d.if_.is_none());
        assert!(d.tss.is_none());
        assert!(d.workout_type.is_none());
    }

    #[test]
    fn derive_metrics_constant_power_np_equals_power() {
        // For a constant power series NP == that constant.
        let powers = vec![200.0_f64; 60];
        let d = derive_metrics(&powers, 250, 3600);
        let np = d.np_w.unwrap();
        // Allow 1 W rounding from f32 precision.
        assert!((np - 200.0).abs() < 1.0, "NP {np} expected ~200 W");
    }

    #[test]
    fn derive_metrics_if_equals_np_over_ftp() {
        let powers = vec![250.0_f64; 60];
        let d = derive_metrics(&powers, 250, 3600);
        let np = d.np_w.unwrap();
        let if_ = d.if_.unwrap();
        let expected_if = np / 250.0;
        assert!((if_ - expected_if).abs() < 1e-4, "IF {if_} expected {expected_if}");
    }

    #[test]
    fn derive_metrics_tss_formula_one_hour_at_ftp() {
        // One hour at FTP: IF = 1.0, TSS = 100.
        let powers = vec![250.0_f64; 3600];
        let d = derive_metrics(&powers, 250, 3600);
        let tss = d.tss.unwrap();
        // TSS = (3600 / 3600) * 1.0^2 * 100 = 100. Allow small f32 rounding.
        assert!((tss - 100.0).abs() < 0.5, "TSS {tss} expected ~100");
    }

    #[test]
    fn derive_metrics_shorter_than_30s_uses_mean_power_as_np() {
        // With fewer than 30 samples normalized_power falls back to mean.
        // Pin that current behavior here so a future refactor cannot silently break it.
        let powers = vec![100.0_f64, 200.0, 300.0]; // mean = 200 W
        let d = derive_metrics(&powers, 200, 3);
        let np = d.np_w.unwrap();
        assert!((np - 200.0).abs() < 1.0, "NP {np} expected ~200 W (mean fallback)");
    }

    #[test]
    fn derive_metrics_classifies_threshold_effort() {
        // 30 min at FTP produces IF ~1.0, which classify sees as Threshold.
        let powers = vec![250.0_f64; 1800];
        let d = derive_metrics(&powers, 250, 1800);
        assert_eq!(d.workout_type, Some(WorkoutType::Threshold));
    }

    #[test]
    fn derive_metrics_tss_scales_with_duration() {
        // Half an hour at FTP: TSS = 0.5 * 1.0 * 100 = 50.
        let powers = vec![250.0_f64; 1800];
        let d = derive_metrics(&powers, 250, 1800);
        let tss = d.tss.unwrap();
        assert!((tss - 50.0).abs() < 0.5, "TSS {tss} expected ~50");
    }
}
