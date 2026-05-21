use crate::errors::AppError;
use crate::errors::AppError::ParseError;
use super::types::{
    Feature, FeatureVal,
    MORE_DATA_FLAG, AVERAGE_SPEED_FLAG, INSTANTANEOUS_CADENCE_FLAG,
    AVERAGE_CADENCE_FLAG, TOTAL_DISTANCE_FLAG, RESISTANCE_LEVEL_FLAG,
    INSTANTANEOUS_POWER_FLAG, AVERAGE_POWER_FLAG, EXPENDED_ENERGY_FLAG,
    HEART_RATE_FLAG, METABOLIC_EQUIVALENT_FLAG, ELAPSED_TIME_FLAG, REMAINING_TIME_FLAG,
};

fn read_u16(data: &[u8], size: usize) -> Result<u16, AppError> {
    if data.len() < size {
        return Err(ParseError("packet too short".into()));
    }
    Ok(u16::from_le_bytes([data[0], data[1]]))
}

fn get_instantaneous_speed(data: &[u8], size: usize) -> Result<FeatureVal, AppError> {
    Ok(FeatureVal::InstantaneousSpeed(read_u16(data, size)? as f32 / 100.0))
}

fn get_average_speed(data: &[u8], size: usize) -> Result<FeatureVal, AppError> {
    Ok(FeatureVal::AvgSpeed(read_u16(data, size)? as f32 / 100.0))
}

fn get_instantaneous_cadence(data: &[u8], size: usize) -> Result<FeatureVal, AppError> {
    Ok(FeatureVal::InstantaneousCadenceRpm(read_u16(data, size)? / 2))
}

fn get_average_cadence(data: &[u8], size: usize) -> Result<FeatureVal, AppError> {
    Ok(FeatureVal::AvgCadenceRpm(read_u16(data, size)? / 2))
}

fn get_total_distance(data: &[u8], size: usize) -> Result<FeatureVal, AppError> {
    if data.len() < size {
        return Err(ParseError("packet too short".into()));
    }
    let val = u32::from_le_bytes([data[0], data[1], data[2], 0]);
    Ok(FeatureVal::TotalDistance(val))
}

fn get_resistance_level(data: &[u8], size: usize) -> Result<FeatureVal, AppError> {
    Ok(FeatureVal::ResistanceLevel(read_u16(data, size)?))
}

fn get_instantaneous_power(data: &[u8], size: usize) -> Result<FeatureVal, AppError> {
    Ok(FeatureVal::InstantaneousPower(read_u16(data, size)? as i16))
}

fn get_average_power(data: &[u8], size: usize) -> Result<FeatureVal, AppError> {
    Ok(FeatureVal::AvgPower(read_u16(data, size)? as i16))
}

fn get_expended_energy(data: &[u8], size: usize) -> Result<FeatureVal, AppError> {
    if data.len() < size {
        return Err(ParseError("packet too short".into()));
    }
    let val = u64::from_le_bytes([data[0], data[1], data[2], data[3], data[4], 0, 0, 0]);
    Ok(FeatureVal::ExpendedEnergy(val))
}

fn get_heart_rate(data: &[u8], size: usize) -> Result<FeatureVal, AppError> {
    if data.len() < size {
        return Err(ParseError("packet too short".into()));
    }
    Ok(FeatureVal::HeartRate(data[0]))
}

fn get_metabolic_equivalent(data: &[u8], size: usize) -> Result<FeatureVal, AppError> {
    if data.len() < size {
        return Err(ParseError("packet too short".into()));
    }
    Ok(FeatureVal::MetabolicEquivalent(data[0]))
}

fn get_elapsed_time(data: &[u8], size: usize) -> Result<FeatureVal, AppError> {
    Ok(FeatureVal::ElapsedTime(read_u16(data, size)?))
}

fn get_remaining_time(data: &[u8], size: usize) -> Result<FeatureVal, AppError> {
    Ok(FeatureVal::RemainingTime(read_u16(data, size)?))
}

const FEATURE_INSTANTANEOUS_SPEED: Feature = Feature { bitmask: MORE_DATA_FLAG, size_bytes: 2, parse: get_instantaneous_speed };
const FEATURE_AVERAGE_SPEED: Feature = Feature { bitmask: AVERAGE_SPEED_FLAG, size_bytes: 2, parse: get_average_speed };
const FEATURE_INSTANTANEOUS_CADENCE: Feature = Feature { bitmask: INSTANTANEOUS_CADENCE_FLAG, size_bytes: 2, parse: get_instantaneous_cadence };
const FEATURE_AVERAGE_CADENCE: Feature = Feature { bitmask: AVERAGE_CADENCE_FLAG, size_bytes: 2, parse: get_average_cadence };
const FEATURE_TOTAL_DISTANCE: Feature = Feature { bitmask: TOTAL_DISTANCE_FLAG, size_bytes: 3, parse: get_total_distance };
const FEATURE_RESISTANCE_LEVEL: Feature = Feature { bitmask: RESISTANCE_LEVEL_FLAG, size_bytes: 2, parse: get_resistance_level };
const FEATURE_INSTANTANEOUS_POWER: Feature = Feature { bitmask: INSTANTANEOUS_POWER_FLAG, size_bytes: 2, parse: get_instantaneous_power };
const FEATURE_AVERAGE_POWER: Feature = Feature { bitmask: AVERAGE_POWER_FLAG, size_bytes: 2, parse: get_average_power };
const FEATURE_EXPENDED_ENERGY: Feature = Feature { bitmask: EXPENDED_ENERGY_FLAG, size_bytes: 5, parse: get_expended_energy };
const FEATURE_HEART_RATE: Feature = Feature { bitmask: HEART_RATE_FLAG, size_bytes: 1, parse: get_heart_rate };
const FEATURE_METABOLIC_EQUIVALENT: Feature = Feature { bitmask: METABOLIC_EQUIVALENT_FLAG, size_bytes: 1, parse: get_metabolic_equivalent };
const FEATURE_ELAPSED_TIME: Feature = Feature { bitmask: ELAPSED_TIME_FLAG, size_bytes: 2, parse: get_elapsed_time };
const FEATURE_REMAINING_TIME: Feature = Feature { bitmask: REMAINING_TIME_FLAG, size_bytes: 2, parse: get_remaining_time };

pub(super) const FEATURES: [Feature; 13] = [
    FEATURE_INSTANTANEOUS_SPEED,
    FEATURE_AVERAGE_SPEED,
    FEATURE_INSTANTANEOUS_CADENCE,
    FEATURE_AVERAGE_CADENCE,
    FEATURE_TOTAL_DISTANCE,
    FEATURE_RESISTANCE_LEVEL,
    FEATURE_INSTANTANEOUS_POWER,
    FEATURE_AVERAGE_POWER,
    FEATURE_EXPENDED_ENERGY,
    FEATURE_HEART_RATE,
    FEATURE_METABOLIC_EQUIVALENT,
    FEATURE_ELAPSED_TIME,
    FEATURE_REMAINING_TIME,
];
