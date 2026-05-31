use crate::errors::AppError;

#[derive(Default)]
pub struct HeartRateMeasurement {
    pub hr_bpm: u16,
}

const HR_VALUE_FORMAT_FLAG: u8 = 1 << 0;

pub fn parse_heart_rate_measurement(packet: &[u8]) -> Result<HeartRateMeasurement, AppError> {
    if packet.len() < 2 {
        return Err(AppError::HRSParseError(String::from("Not enough packet")));
    }
    let flags = packet[0];
    if flags & HR_VALUE_FORMAT_FLAG == 0 {
        return Ok(HeartRateMeasurement {
            hr_bpm: packet[1] as u16,
        });
    }
    if packet.len() < 3 {
        return Err(AppError::HRSParseError(
            "16-bit HR value requires 3 bytes".into(),
        ));
    }
    Ok(HeartRateMeasurement {
        hr_bpm: u16::from_le_bytes([packet[1], packet[2]]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::AppError::HRSParseError;

    #[test]
    fn test_parse_heart_rate_measurement_given_invalid_packet_should_return_parse_error() {
        let packet: &[u8] = &[];
        let res = parse_heart_rate_measurement(packet);
        assert!(res.is_err());
        assert!(matches!(res, Err(HRSParseError(_))));
    }

    #[test]
    fn test_parse_heart_rate_measurement_given_valid_packet_should_return_hr_measurement(
    ) -> Result<(), AppError> {
        let packet: &[u8] = &[0x10, 0x4B];
        let res = parse_heart_rate_measurement(packet)?;
        assert_eq!(res.hr_bpm, 75);
        Ok(())
    }

    #[test]
    fn test_parse_heart_rate_measurement_given_16bit_packet_should_return_hr_measurement(
    ) -> Result<(), AppError> {
        let packet: &[u8] = &[0x01, 0x2C, 0x01];
        let res = parse_heart_rate_measurement(packet)?;
        assert_eq!(res.hr_bpm, 300);
        Ok(())
    }
}
