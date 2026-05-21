use crate::errors::AppError;

#[derive(Default)]
pub struct IndoorBikeData {
    pub instantaneous_speed_kmh: Option<f32>,
    pub avg_speed_kmh: Option<f32>,
    pub instantaneous_cadence_rpm: Option<u16>,
    pub avg_cadence_rpm: Option<u16>,
    pub total_distance_m: Option<u32>,
    pub resistance_level: Option<u16>,
    pub instantaneous_power_w: Option<i16>,
    pub avg_power_w: Option<i16>,
    pub expended_energy_kcal: Option<u64>,
    pub heart_rate_bpm: Option<u8>,
    pub metabolic_equivalent: Option<u8>,
    pub elapsed_time_s: Option<u16>,
    pub remaining_time_s: Option<u16>,
}

pub(crate) const FLAGS_LEN: usize = 2;

// Bit 0: More Data — when 0, Instantaneous Speed is present (inverted logic)
pub(crate) const MORE_DATA_FLAG: u16 = 1 << 0;
pub(crate) const AVERAGE_SPEED_FLAG: u16 = 1 << 1;
pub(crate) const INSTANTANEOUS_CADENCE_FLAG: u16 = 1 << 2;
pub(crate) const AVERAGE_CADENCE_FLAG: u16 = 1 << 3;
pub(crate) const TOTAL_DISTANCE_FLAG: u16 = 1 << 4;
pub(crate) const RESISTANCE_LEVEL_FLAG: u16 = 1 << 5;
pub(crate) const INSTANTANEOUS_POWER_FLAG: u16 = 1 << 6;
pub(crate) const AVERAGE_POWER_FLAG: u16 = 1 << 7;
pub(crate) const EXPENDED_ENERGY_FLAG: u16 = 1 << 8;
pub(crate) const HEART_RATE_FLAG: u16 = 1 << 9;
pub(crate) const METABOLIC_EQUIVALENT_FLAG: u16 = 1 << 10;
pub(crate) const ELAPSED_TIME_FLAG: u16 = 1 << 11;
pub(crate) const REMAINING_TIME_FLAG: u16 = 1 << 12;

pub(crate) enum FeatureVal {
    InstantaneousSpeed(f32),
    AvgSpeed(f32),
    InstantaneousCadenceRpm(u16),
    AvgCadenceRpm(u16),
    TotalDistance(u32),
    ResistanceLevel(u16),
    InstantaneousPower(i16),
    AvgPower(i16),
    ExpendedEnergy(u64),
    HeartRate(u8),
    MetabolicEquivalent(u8),
    ElapsedTime(u16),
    RemainingTime(u16),
}

pub(crate) struct Feature {
    pub(crate) bitmask: u16,
    pub(crate) size_bytes: usize,
    pub(crate) parse: fn(&[u8], usize) -> Result<FeatureVal, AppError>,
}
