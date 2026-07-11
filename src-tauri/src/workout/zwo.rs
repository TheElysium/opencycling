use crate::errors::AppError;
use crate::workout::types::{ParsedWorkout, SportType, WorkoutBlock};
use roxmltree::Node;

pub(crate) fn parse_zwo(file_content: &str) -> Result<ParsedWorkout, AppError> {
    let doc = roxmltree::Document::parse(file_content)
        .map_err(|e| AppError::ZWOFileParseError(e.to_string()))?;

    let root = doc.root_element();
    let author = zwo_metadata_text(root, "author");
    let name = zwo_metadata_text(root, "name");
    let description = zwo_metadata_text(root, "description");
    let sport_type = match root
        .children()
        .find(|n| n.tag_name().name() == "sportType")
        .and_then(|n| n.text())
    {
        Some("running") => SportType::Running,
        Some("bike") => SportType::Bike,
        None => return Err(AppError::ZWOFileParseError("no sport specified".into())),
        _ => return Err(AppError::ZWOFileParseError("unknown sport type".into())),
    };

    let workout_blocks = root
        .children()
        .find(|n| n.tag_name().name() == "workout")
        .ok_or_else(|| AppError::ZWOFileParseError("missing workout block".to_string()))?;

    let mut parsed_blocks: Vec<WorkoutBlock> = Vec::new();

    for block in workout_blocks.children().filter(Node::is_element) {
        match block.tag_name().name() {
            "Warmup" => {
                parsed_blocks.push(ramp_to_workout_block(block, Some("Warmup"))?);
            }
            "SteadyState" => parsed_blocks.push(steady_state_to_workout_block(block)?),
            "IntervalsT" => parsed_blocks.push(intervals_t_to_workout_blocks(block)?),
            "Ramp" => parsed_blocks.push(ramp_to_workout_block(block, None)?),
            "Cooldown" => parsed_blocks.push(ramp_to_workout_block(block, Some("Cooldown"))?),
            // FreeRide: intentionally skipped until product work adds free-ride support.
            "FreeRide" => {}
            tag => {
                tracing::warn!("zwo: skipping unknown block type \"{}\"", tag);
            }
        }
    }

    Ok(ParsedWorkout {
        author,
        name,
        description,
        sport_type,
        workout_blocks: parsed_blocks,
        is_ftp_test: has_ftp_test_tag(root),
        file_name: None,
    })
}

fn has_ftp_test_tag(root: Node) -> bool {
    root.children()
        .find(|n| n.has_tag_name("tags"))
        .map(|tags| {
            tags.children().filter(Node::is_element).any(|t| {
                t.has_tag_name("tag")
                    && t.attribute("name")
                        .map(|v| v.eq_ignore_ascii_case("ftp-test"))
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn zwo_metadata_text(node: Node, tag: &str) -> Option<String> {
    node.children()
        .find(|n| n.has_tag_name(tag))
        .and_then(|n| n.text())
        .map(str::to_string)
}

fn intervals_t_to_workout_blocks(intervals_t_node: Node) -> Result<WorkoutBlock, AppError> {
    let intervals_nbr = intervals_t_node
        .attribute("Repeat")
        .ok_or_else(|| AppError::ZWOFileParseError("missing interval repeat nbr".to_string()))?
        .parse::<u16>()
        .map_err(|_| {
            AppError::ZWOFileParseError("interval repeat nbr parsing error".to_string())
        })?;

    Ok(WorkoutBlock::IntervalsT {
        repeat: intervals_nbr,
        on: Box::from(intervals_t_to_on_block(intervals_t_node)?),
        off: Box::from(intervals_t_to_off_block(intervals_t_node)?),
    })
}

fn intervals_t_to_on_block(node: Node) -> Result<WorkoutBlock, AppError> {
    let duration_s = read_duration(node, "OnDuration")?;
    let power_pct = read_power(node, "OnPower")?;
    let cadence_rpm = read_cadence(node, "Cadence")?;

    Ok(WorkoutBlock::SteadyState {
        duration_s,
        power_pct,
        cadence_rpm,
        label: None,
    })
}

fn intervals_t_to_off_block(node: Node) -> Result<WorkoutBlock, AppError> {
    let duration_s = read_duration(node, "OffDuration")?;
    let power_pct = read_power(node, "OffPower")?;
    let cadence_rpm = read_cadence(node, "CadenceResting")?;

    Ok(WorkoutBlock::SteadyState {
        duration_s,
        power_pct,
        cadence_rpm,
        label: None,
    })
}

fn steady_state_to_workout_block(steady_state_block: Node) -> Result<WorkoutBlock, AppError> {
    Ok(WorkoutBlock::SteadyState {
        duration_s: read_duration(steady_state_block, "Duration")?,
        power_pct: read_power(steady_state_block, "Power")?,
        cadence_rpm: read_cadence(steady_state_block, "Cadence")?,
        label: read_label(steady_state_block),
    })
}

fn ramp_to_workout_block(
    node: Node,
    default_label: Option<&str>,
) -> Result<WorkoutBlock, AppError> {
    Ok(WorkoutBlock::Ramp {
        duration_s: read_duration(node, "Duration")?,
        power_start_pct: read_power(node, "PowerLow")?,
        power_end_pct: read_power(node, "PowerHigh")?,
        cadence_rpm: read_cadence(node, "Cadence")?,
        label: read_label(node).or_else(|| default_label.map(str::to_string)),
    })
}

fn read_duration(node: Node, attribute_name: &str) -> Result<u32, AppError> {
    node.attribute(attribute_name)
        .ok_or_else(|| {
            AppError::ZWOFileParseError(format!("workout_block missing {attribute_name}"))
        })?
        .parse::<u32>()
        .map_err(|e| AppError::ZWOFileParseError(format!("invalid {attribute_name}: {e}")))
}

fn read_power(node: Node, attribute_name: &str) -> Result<f32, AppError> {
    node.attribute(attribute_name)
        .ok_or_else(|| {
            AppError::ZWOFileParseError(format!("workout_block missing {attribute_name}"))
        })?
        .parse::<f32>()
        .map_err(|e| AppError::ZWOFileParseError(format!("invalid {attribute_name}: {e}")))
}

fn read_cadence(node: Node, attribute_name: &str) -> Result<Option<u16>, AppError> {
    match node.attribute(attribute_name) {
        Some(s) => s
            .parse::<u16>()
            .map_err(|e| AppError::ZWOFileParseError(format!("invalid {attribute_name}: {e}")))
            .map(Some),
        None => Ok(None),
    }
}
fn read_label(node: Node) -> Option<String> {
    node.attribute("name").map(|label| label.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::AppError::ZWOFileParseError;
    use crate::workout::types::SportType::Bike;
    use std::fs;

    const ALL_BLOCK_TYPES_PATH: &str = "tests/fixtures/all_block_types.zwo";
    const THRESHOLD_3X3_PATH: &str = "tests/fixtures/threshold_3x3.zwo";

    fn parse_fixture(path: &str) -> Result<ParsedWorkout, AppError> {
        let content = fs::read_to_string(path).unwrap();
        parse_zwo(&content)
    }

    #[test]
    fn test_parse_zwo_given_valid_zwo_return_() -> Result<(), AppError> {
        let parsed_workout = parse_fixture(ALL_BLOCK_TYPES_PATH)?;
        assert_eq!(parsed_workout.author, Some("OpenCycling".to_string()));
        assert_eq!(parsed_workout.name, Some("Test Workout".to_string()));
        assert_eq!(
            parsed_workout.description,
            Some("A test workout covering all block types".to_string())
        );
        assert_eq!(parsed_workout.sport_type, Bike);
        assert_eq!(parsed_workout.workout_blocks.len(), 4);

        let content = fs::read_to_string(THRESHOLD_3X3_PATH).unwrap();
        let workout = parse_zwo(&content)?;
        assert_eq!(workout.workout_blocks.len(), 6);

        Ok(())
    }

    #[test]
    fn test_parse_ftp_test_tag_sets_flag() -> Result<(), AppError> {
        let xml = r#"<workout_file><sportType>bike</sportType><tags><tag name="ftp-test"/></tags><workout><SteadyState Duration="60" Power="1.0"/></workout></workout_file>"#;
        assert!(parse_zwo(xml)?.is_ftp_test);
        Ok(())
    }

    #[test]
    fn test_parse_without_ftp_test_tag_is_false() -> Result<(), AppError> {
        let xml = r#"<workout_file><sportType>bike</sportType><workout><SteadyState Duration="300" Power="0.85"/></workout></workout_file>"#;
        assert!(!parse_zwo(xml)?.is_ftp_test);
        Ok(())
    }

    #[test]
    fn test_parse_steady_state_values() -> Result<(), AppError> {
        let xml = r#"<workout_file><sportType>bike</sportType><workout><SteadyState Duration="300" Power="0.85"/></workout></workout_file>"#;
        let workout = parse_zwo(xml)?;
        match &workout.workout_blocks[0] {
            WorkoutBlock::SteadyState {
                duration_s,
                power_pct,
                ..
            } => {
                assert_eq!(*duration_s, 300);
                assert!((power_pct - 0.85).abs() < 0.001);
            }
            _ => panic!("Expected SteadyState"),
        }
        Ok(())
    }

    #[test]
    fn test_parse_ramp_values() -> Result<(), AppError> {
        let xml = r#"<workout_file><sportType>bike</sportType><workout><Warmup Duration="600" PowerLow="0.40" PowerHigh="0.75"/></workout></workout_file>"#;
        let workout = parse_zwo(xml)?;
        match &workout.workout_blocks[0] {
            WorkoutBlock::Ramp {
                duration_s,
                power_start_pct,
                power_end_pct,
                ..
            } => {
                assert_eq!(*duration_s, 600);
                assert!((power_start_pct - 0.40).abs() < 0.001);
                assert!((power_end_pct - 0.75).abs() < 0.001);
            }
            _ => panic!("Expected Ramp"),
        }
        Ok(())
    }

    #[test]
    fn test_parse_intervals_t_values() -> Result<(), AppError> {
        let xml = r#"<workout_file><sportType>bike</sportType><workout><IntervalsT Repeat="2" OnDuration="180" OffDuration="120" OnPower="1.10" OffPower="0.55"/></workout></workout_file>"#;
        let workout = parse_zwo(xml)?;
        assert_eq!(workout.workout_blocks.len(), 1);
        match &workout.workout_blocks[0] {
            WorkoutBlock::IntervalsT { repeat, on, off } => {
                assert_eq!(*repeat, 2);
                match on.as_ref() {
                    WorkoutBlock::SteadyState {
                        duration_s,
                        power_pct,
                        ..
                    } => {
                        assert_eq!(*duration_s, 180);
                        assert!((power_pct - 1.10).abs() < 0.001);
                    }
                    _ => panic!("Expected SteadyState for ON block"),
                }
                match off.as_ref() {
                    WorkoutBlock::SteadyState {
                        duration_s,
                        power_pct,
                        ..
                    } => {
                        assert_eq!(*duration_s, 120);
                        assert!((power_pct - 0.55).abs() < 0.001);
                    }
                    _ => panic!("Expected SteadyState for OFF block"),
                }
            }
            _ => panic!("Expected IntervalsT"),
        }
        Ok(())
    }

    #[test]
    fn test_parse_zwo_given_empty_returns_error() {
        assert!(matches!(parse_zwo(""), Err(ZWOFileParseError(_))));
    }

    #[test]
    fn test_parse_missing_sport_type_returns_error() {
        assert!(matches!(
            parse_fixture("tests/fixtures/errors/missing_sport_type.zwo"),
            Err(ZWOFileParseError(_))
        ));
    }

    #[test]
    fn test_parse_unknown_sport_type_returns_error() {
        assert!(matches!(
            parse_fixture("tests/fixtures/errors/unknown_sport_type.zwo"),
            Err(ZWOFileParseError(_))
        ));
    }

    #[test]
    fn test_parse_missing_workout_section_returns_error() {
        assert!(matches!(
            parse_fixture("tests/fixtures/errors/missing_workout_section.zwo"),
            Err(ZWOFileParseError(_))
        ));
    }

    #[test]
    fn test_parse_steady_state_missing_power_returns_error() {
        assert!(matches!(
            parse_fixture("tests/fixtures/errors/steady_state_missing_power.zwo"),
            Err(ZWOFileParseError(_))
        ));
    }

    #[test]
    fn test_parse_intervals_missing_repeat_returns_error() {
        assert!(matches!(
            parse_fixture("tests/fixtures/errors/intervals_missing_repeat.zwo"),
            Err(ZWOFileParseError(_))
        ));
    }

    #[test]
    fn test_parse_invalid_duration_returns_error() {
        assert!(matches!(
            parse_fixture("tests/fixtures/errors/invalid_duration.zwo"),
            Err(ZWOFileParseError(_))
        ));
    }

    #[test]
    fn test_parse_ramp_block_produces_ramp() -> Result<(), AppError> {
        let xml = r#"<workout_file><sportType>bike</sportType><workout><Ramp Duration="300" PowerLow="0.50" PowerHigh="0.80"/></workout></workout_file>"#;
        let workout = parse_zwo(xml)?;
        assert_eq!(workout.workout_blocks.len(), 1);
        match &workout.workout_blocks[0] {
            WorkoutBlock::Ramp {
                duration_s,
                power_start_pct,
                power_end_pct,
                cadence_rpm,
                label,
            } => {
                assert_eq!(*duration_s, 300);
                assert!((power_start_pct - 0.50).abs() < 0.001);
                assert!((power_end_pct - 0.80).abs() < 0.001);
                assert_eq!(*cadence_rpm, None);
                assert_eq!(*label, None);
            }
            _ => panic!("Expected Ramp block"),
        }
        Ok(())
    }

    #[test]
    fn test_parse_ramp_block_with_cadence_and_name() -> Result<(), AppError> {
        let xml = r#"<workout_file><sportType>bike</sportType><workout><Ramp Duration="120" PowerLow="0.60" PowerHigh="0.90" Cadence="85" name="Build"/></workout></workout_file>"#;
        let workout = parse_zwo(xml)?;
        match &workout.workout_blocks[0] {
            WorkoutBlock::Ramp {
                cadence_rpm,
                label,
                ..
            } => {
                assert_eq!(*cadence_rpm, Some(85));
                assert_eq!(label.as_deref(), Some("Build"));
            }
            _ => panic!("Expected Ramp block"),
        }
        Ok(())
    }

    #[test]
    fn test_parse_unknown_block_type_does_not_error_and_produces_no_blocks() -> Result<(), AppError>
    {
        let xml = r#"<workout_file><sportType>bike</sportType><workout><UnknownFutureBlock Duration="60" Power="0.75"/><SteadyState Duration="300" Power="0.85"/></workout></workout_file>"#;
        let workout = parse_zwo(xml)?;
        // Only the SteadyState should be in the output; the unknown tag is skipped.
        assert_eq!(workout.workout_blocks.len(), 1);
        assert!(matches!(workout.workout_blocks[0], WorkoutBlock::SteadyState { .. }));
        Ok(())
    }

    #[test]
    fn test_parse_free_ride_is_intentionally_skipped() -> Result<(), AppError> {
        let xml = r#"<workout_file><sportType>bike</sportType><workout><FreeRide Duration="600"/><SteadyState Duration="300" Power="0.75"/></workout></workout_file>"#;
        let workout = parse_zwo(xml)?;
        // FreeRide is an explicit intentional skip; only the SteadyState appears.
        assert_eq!(workout.workout_blocks.len(), 1);
        assert!(matches!(workout.workout_blocks[0], WorkoutBlock::SteadyState { .. }));
        Ok(())
    }
}
