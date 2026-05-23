mod features;
mod types;

use crate::errors::AppError;
use crate::errors::AppError::FTMSPacketParseError;
use features::FEATURES;
use types::{FeatureVal, FLAGS_LEN, MORE_DATA_FLAG};

pub use types::IndoorBikeData;

fn feature_enabled(bitmask: u16, flags: u16) -> bool {
    flags & bitmask == bitmask
}

fn speed_enabled(bitmask: u16, flags: u16) -> bool {
    bitmask == MORE_DATA_FLAG && flags & MORE_DATA_FLAG == 0
}

pub fn parse_indoor_bike_data(packet: &[u8]) -> Result<IndoorBikeData, AppError> {
    if packet.len() < FLAGS_LEN {
        return Err(FTMSPacketParseError("packet too short".into()));
    }

    let flags = u16::from_le_bytes([packet[0], packet[1]]);
    let mut enabled_features = vec![];
    for f in FEATURES {
        if speed_enabled(f.bitmask, flags) || feature_enabled(f.bitmask, flags) {
            enabled_features.push(f);
        }
    }

    let mut result = IndoorBikeData::default();
    let payload = &packet[FLAGS_LEN..];
    let mut offset = 0;
    for feature in enabled_features {
        match (feature.parse)(&payload[offset..], feature.size_bytes) {
            Ok(FeatureVal::InstantaneousSpeed(v)) => result.instantaneous_speed_kmh = Some(v),
            Ok(FeatureVal::AvgSpeed(v)) => result.avg_speed_kmh = Some(v),
            Ok(FeatureVal::InstantaneousCadenceRpm(v)) => result.instantaneous_cadence_rpm = Some(v),
            Ok(FeatureVal::AvgCadenceRpm(v)) => result.avg_cadence_rpm = Some(v),
            Ok(FeatureVal::TotalDistance(v)) => result.total_distance_m = Some(v),
            Ok(FeatureVal::ResistanceLevel(v)) => result.resistance_level = Some(v),
            Ok(FeatureVal::InstantaneousPower(v)) => result.instantaneous_power_w = Some(v),
            Ok(FeatureVal::AvgPower(v)) => result.avg_power_w = Some(v),
            Ok(FeatureVal::ExpendedEnergy(v)) => result.expended_energy_kcal = Some(v),
            Ok(FeatureVal::HeartRate(v)) => result.heart_rate_bpm = Some(v),
            Ok(FeatureVal::MetabolicEquivalent(v)) => result.metabolic_equivalent = Some(v),
            Ok(FeatureVal::ElapsedTime(v)) => result.elapsed_time_s = Some(v),
            Ok(FeatureVal::RemainingTime(v)) => result.remaining_time_s = Some(v),
            Err(_) => {}
        }
        offset += feature.size_bytes;
    }
    Ok(result)
}

const SET_TARGET_POWER_OPCODE: u8 = 0x05;
pub fn build_set_target_power_command(watts: i16) -> [u8;3] {
    let [b0, b1] = watts.to_le_bytes();
    [SET_TARGET_POWER_OPCODE,b0, b1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_indoor_bike_data_given_empty_packet_should_return_parse_error() {
        let packet: &[u8] = &[];
        let res = parse_indoor_bike_data(packet);
        assert!(res.is_err());
        assert!(matches!(res, Err(FTMSPacketParseError(_))));
    }

    #[test]
    fn test_parse_indoor_bike_data_given_packet_should_return_indoor_bike_data() -> Result<(), AppError> {
        let packet: &[u8] = &[0x44, 0x00, 0x48, 0x09, 0x64, 0x00, 0xA0, 0x00, 0x82, 0x05, 0xF2, 0x13, 0x38, 0x06, 0xD3, 0x00, 0x40, 0x03];
        let indoor_bike_data = parse_indoor_bike_data(packet)?;
        assert_eq!(indoor_bike_data.instantaneous_speed_kmh, Some(23.76_f32));
        assert_eq!(indoor_bike_data.instantaneous_cadence_rpm, Some(50_u16));
        assert_eq!(indoor_bike_data.instantaneous_power_w, Some(160_i16));
        Ok(())
    }

    #[test]
    fn test_build_set_target_power_command_given_power_w_should_return_command() {
        let expected_watts:i16 = 250;
        let res = build_set_target_power_command(expected_watts);
        assert_eq!(res[0], SET_TARGET_POWER_OPCODE);
        let got_watts = i16::from_le_bytes([res[1], res[2]]);
        assert_eq!(expected_watts, got_watts);

        let expected_watts:i16 = 0;
        let res = build_set_target_power_command(expected_watts);
        assert_eq!(res[0], SET_TARGET_POWER_OPCODE);
        let got_watts = i16::from_le_bytes([res[1], res[2]]);
        assert_eq!(expected_watts, got_watts);
    }
}
