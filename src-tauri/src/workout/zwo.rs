use roxmltree::Node;
use crate::errors::AppError;

#[derive(Debug)]
pub struct ParsedWorkout {
    pub author: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub sport_type: SportType,
    pub workout_blocks: Vec<WorkoutBlock>,
}
#[derive(Debug, Clone)]
pub enum WorkoutBlock {
    SteadyState{duration_s: u32, power_pct: f32, cadence_rpm: Option<u16>, label: Option<String>},
    Ramp{duration_s: u32, power_start_pct: f32, power_end_pct: f32, cadence_rpm: Option<u16>, label: Option<String>},
}

#[derive(Debug, PartialEq, Eq)]
pub enum SportType {
    Bike,
    Running,
}
pub fn parse_zwo(file_content: &str) -> Result<ParsedWorkout, AppError> {
    let doc = roxmltree::Document::parse(file_content)
        .map_err(|e| AppError::ZWOFileParseError(e.to_string()))?;

    let root = doc.root_element();

    let author = root.children()
        .find(|n| n.tag_name().name() == "author")
        .and_then(|n| n.text())
        .map(str::to_string);

    let name = root.children()
        .find(|n| n.tag_name().name() == "name")
        .and_then(|n| n.text())
        .map(str::to_string);
    let description = root.children()
        .find(|n| n.tag_name().name() == "description")
        .and_then(|n| n.text())
        .map(str::to_string);
    let sport_type = match root.children()
        .find(|n| n.tag_name().name() == "sportType")
        .and_then(|n| n.text())
    {
        Some("running") => SportType::Running,
        Some("bike") => SportType::Bike,
        None => return Err(AppError::ZWOFileParseError("no sport specified".into())),
        _ => return Err(AppError::ZWOFileParseError("unknown sport type".into())),
    };

    let workout_blocks = root.children()
        .find(|n| n.tag_name().name() == "workout")
        .ok_or_else(|| AppError::ZWOFileParseError("missing workout block".to_string()))?;


    let mut parsed_blocks: Vec<WorkoutBlock> = Vec::new();

   for block in  workout_blocks.children() {
       match block.tag_name().name().to_lowercase().as_str() {
           "warmup"=> {
               parsed_blocks.push(ramp_to_workout_block(block)?);
           }
           "steadystate"=>{
               parsed_blocks.push(steady_state_to_workout_block(block)?)
           }
           "intervalst"=>{
               parsed_blocks.extend(intervals_t_to_workout_blocks(block)?)
           }
           "freeride"=>{}
           "cooldown"=>{
               parsed_blocks.push(ramp_to_workout_block(block)?)
           },
           &_ => {}
       }
   }

    Ok(ParsedWorkout {
        author,
        name,
        description,
        sport_type,
        workout_blocks: parsed_blocks,
    })
}

fn intervals_t_to_workout_blocks(intervals_t_node: Node) -> Result<Vec<WorkoutBlock>, AppError> {
    let intervals_nbr = intervals_t_node.attribute("Repeat")
        .ok_or_else(|| AppError::ZWOFileParseError("missing interval repeat nbr".to_string()))?
        .parse::<u16>()
        .map_err(|_| AppError::ZWOFileParseError("interval repeat nbr parsing error".to_string()))?;

    let mut workout_blocks: Vec<WorkoutBlock> = Vec::new();
    let on_block = intervals_t_to_on_block(intervals_t_node)?;
    let off_block = intervals_t_to_off_block(intervals_t_node)?;
    for _ in 0..intervals_nbr {
        workout_blocks.push(on_block.clone());
        workout_blocks.push(off_block.clone());
    }
    Ok(workout_blocks)
}

fn intervals_t_to_on_block(node: Node) -> Result<WorkoutBlock, AppError> {
    let duration_s = read_duration(node, "OnDuration")?;
    let power_pct = read_power(node, "OnPower")?;
    let cadence_rpm = read_cadence(node, "OnCadence")?;

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
    let cadence_rpm = read_cadence(node, "OffCadence")?;

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

fn ramp_to_workout_block(warmup_node: Node) -> Result<WorkoutBlock, AppError> {
    Ok(WorkoutBlock::Ramp{
        duration_s: read_duration(warmup_node, "Duration")?,
        power_start_pct: read_power(warmup_node, "PowerLow")?,
        power_end_pct: read_power(warmup_node, "PowerHigh")?,
        cadence_rpm: read_cadence(warmup_node, "Cadence")?,
        label: read_label(warmup_node),
    })
}

fn read_duration(node: Node, attribute_name: &str) -> Result<u32, AppError> {
    node.attribute(attribute_name)
        .ok_or_else(|| AppError::ZWOFileParseError(format!("workout_block missing {attribute_name}")))?
        .parse::<u32>()
        .map_err(|e| AppError::ZWOFileParseError(format!("invalid {attribute_name}: {e}")))
}

fn read_power(node: Node, attribute_name: &str) -> Result<f32, AppError> {
    node.attribute(attribute_name)
        .ok_or_else(|| AppError::ZWOFileParseError(format!("workout_block missing {attribute_name}")))?
        .parse::<f32>()
        .map_err(|e| AppError::ZWOFileParseError(format!("invalid {attribute_name}: {e}")))
}

fn read_cadence(node: Node, attribute_name: &str) -> Result<Option<u16>, AppError> {
    match node.attribute(attribute_name) {
        Some(s) => s.parse::<u16>()
            .map_err(|e| AppError::ZWOFileParseError(format!("invalid {attribute_name}: {e}")))
            .map(Some),
        None => Ok(None),
    }
}
fn read_label(node: Node) -> Option<String> {
    match node.attribute("name") {
        Some(label) => Some(label.to_string()),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use crate::errors::AppError::ZWOFileParseError;
    use crate::workout::zwo::SportType::Bike;
    use super::*;

    const ALL_BLOCK_TYPES_PATH: &str = "tests/fixtures/all_block_types.zwo";
    const THRESHOLD_3X3_PATH: &str = "tests/fixtures/threshold_3x3.zwo";

    #[test]
    fn test_parse_zwo_given_valid_zwo_return_() -> Result<(), AppError>{
        let file_content = fs::read_to_string(ALL_BLOCK_TYPES_PATH);
        assert!(file_content.is_ok());
        let parsed_workout = parse_zwo(&file_content.unwrap())?;
        assert_eq!(parsed_workout.author, Some("OpenCycling".to_string()));
        assert_eq!(parsed_workout.name, Some("Test Workout".to_string()));
        assert_eq!(parsed_workout.description, Some("A test workout covering all block types".to_string()));
        assert_eq!(parsed_workout.sport_type, Bike);
        assert_eq!(parsed_workout.workout_blocks.len(), 11);
        Ok(())
    }

    #[test]
    fn test_parse_fixture_9_returns_14_blocks() -> Result<(), AppError> {
        let content = fs::read_to_string(THRESHOLD_3X3_PATH).unwrap();
        let workout = parse_zwo(&content)?;
        assert_eq!(workout.workout_blocks.len(), 14);
        Ok(())
    }

    #[test]
    fn test_parse_steady_state_values() -> Result<(), AppError> {
        let xml = r#"<workout_file><sportType>bike</sportType><workout><SteadyState Duration="300" Power="0.85"/></workout></workout_file>"#;
        let workout = parse_zwo(xml)?;
        match &workout.workout_blocks[0] {
            WorkoutBlock::SteadyState { duration_s, power_pct, .. } => {
                assert_eq!(*duration_s, 300);
                assert!((power_pct - 0.85).abs() < 0.001);
            }
            _ => panic!("Expected SteadyState"),
        }
        Ok(())
    }

    fn parse_fixture(path: &str) -> Result<ParsedWorkout, AppError> {
        let content = fs::read_to_string(path).unwrap();
        parse_zwo(&content)
    }

    #[test]
    fn test_parse_zwo_given_empty_returns_error() {
        assert!(matches!(parse_zwo(""), Err(ZWOFileParseError(_))));
    }

    #[test]
    fn test_parse_missing_sport_type_returns_error() {
        assert!(matches!(parse_fixture("tests/fixtures/errors/missing_sport_type.zwo"), Err(ZWOFileParseError(_))));
    }

    #[test]
    fn test_parse_unknown_sport_type_returns_error() {
        assert!(matches!(parse_fixture("tests/fixtures/errors/unknown_sport_type.zwo"), Err(ZWOFileParseError(_))));
    }

    #[test]
    fn test_parse_missing_workout_section_returns_error() {
        assert!(matches!(parse_fixture("tests/fixtures/errors/missing_workout_section.zwo"), Err(ZWOFileParseError(_))));
    }

    #[test]
    fn test_parse_steady_state_missing_power_returns_error() {
        assert!(matches!(parse_fixture("tests/fixtures/errors/steady_state_missing_power.zwo"), Err(ZWOFileParseError(_))));
    }

    #[test]
    fn test_parse_intervals_missing_repeat_returns_error() {
        assert!(matches!(parse_fixture("tests/fixtures/errors/intervals_missing_repeat.zwo"), Err(ZWOFileParseError(_))));
    }

    #[test]
    fn test_parse_invalid_duration_returns_error() {
        assert!(matches!(parse_fixture("tests/fixtures/errors/invalid_duration.zwo"), Err(ZWOFileParseError(_))));
    }
}