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
        .ok_or_else(|| AppError::ZWOFileParseError("workout_block missing Duration".to_string()))?
        .parse::<u32>()
        .map_err(|e| AppError::ZWOFileParseError(format!("invalid PowerLow: {e}")))
}

fn read_power(node: Node, attribute_name: &str) -> Result<f32, AppError> {
    node.attribute(attribute_name)
        .ok_or_else(|| AppError::ZWOFileParseError("workout_block missing Power".to_string()))?
        .parse::<f32>()
        .map_err(|e| AppError::ZWOFileParseError(format!("invalid Power: {e}")))
}

fn read_cadence(node: Node, attribute_name: &str) -> Result<Option<u16>, AppError> {
    match node.attribute(attribute_name) {
        Some(s) => s.parse::<u16>()
            .map_err(|e| AppError::ZWOFileParseError(format!("invalid Cadence: {e}")))
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

    const VALID_ZWO_FILE_PATH: &str = "tests/fixtures/test_workout.zwo";
    #[test]
    fn test_parse_zwo_given_valid_zwo_return_() -> Result<(), AppError>{
        let file_content = fs::read_to_string(VALID_ZWO_FILE_PATH);
        assert!(file_content.is_ok());
        let parsed_workout = parse_zwo(&file_content.unwrap())?;
        assert_eq!(parsed_workout.author, Some("OpenCycling".to_string()));
        assert_eq!(parsed_workout.name, Some("Test Workout".to_string()));
        assert_eq!(parsed_workout.description, Some("A test workout covering all block types".to_string()));
        assert_eq!(parsed_workout.sport_type, Bike);
        assert_eq!(parsed_workout.workout_blocks.len(), 4);
        Ok(())
    }

    #[test]
    fn test_parse_zwo_given_empty_zwo_return_() {
        let parsed_workout = parse_zwo("");
        assert!(parsed_workout.is_err());
        assert!(matches!(parsed_workout, Err(ZWOFileParseError(_))));
    }
}