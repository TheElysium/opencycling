use std::str::FromStr;
use roxmltree::Node;
use tauri::App;
use crate::errors::AppError;

#[derive(Debug)]
pub struct ParsedWorkout {
    pub author: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub sport_type: SportType,
    pub workout_blocks: Vec<WorkoutBlock>,
}
#[derive(Debug)]
pub enum WorkoutBlock {
    SteadyState{duration_s: u32, power_pct: f32, cadence_rpm: Option<u16>, label: Option<String>},
    Ramp{duration_s: u32, power_start_pct: f32, power_end_pct: f32, cadence_rpm: Option<u16>, label: Option<String>},
}

#[derive(Debug)]
pub enum SportType {
    Bike,
    Running,
}
pub fn parse_zwo(file_content: &str) -> Result<ParsedWorkout, AppError> {
    let doc = roxmltree::Document::parse(file_content)
        .map_err(|e| AppError::ParseError(e.to_string()))?;

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
        None => return Err(AppError::ParseError("no sport specified".into())),
        _ => return Err(AppError::ParseError("unknown sport type".into())),
    };

    let workout_blocks = root.children()
        .find(|n| n.tag_name().name() == "workout")
        .ok_or_else(|| AppError::ParseError("missing workout block".to_string()))?;


    let mut parsed_blocks: Vec<WorkoutBlock> = Vec::new();

   for block in  workout_blocks.children() {
       match block.tag_name().name().to_lowercase().as_str() {
           "warmup"=> {
               parsed_blocks.push(warmup_to_workout_block(block)?);
           }
           "steadystate"=>{
               parsed_blocks.push(steady_state_to_workout_block(block)?)
           }
           "intervalst"=>{
               parsed_blocks.push(intervals_t_to_workout_blocks(block)?)
           }
           "freeride"=>{}
           "cooldown"=>{},
           &_ => {}
       }
   }



    Ok(ParsedWorkout {
        author,
        name,
        description,
        tags: None,
        sport_type,
        workout_blocks: parsed_blocks,
    })
}

fn intervals_t_to_workout_blocks(intervals_t_node: Node) -> Result<Vec<WorkoutBlock>, AppError> {
    let intervals_nbr = intervals_t_node.attribute("Repeat")
        .ok_or_else(|| AppError::ParseError("missing interval repeat nbr".to_string()))?
        .parse::<u16>()
        .map_err(|_| AppError::ParseError("interval repeat nbr parsing error".to_string()))?;

    let mut workout_blocks: Vec<WorkoutBlock> = Vec::new();
    let on_block = intervals_t_to_on_block(intervals_t_node)?;
    let off_block = intervals_t_to_off_block(intervals_t_node)?;
    Ok(workout_blocks)
}

fn intervals_t_to_on_block(node: Node) -> Result<WorkoutBlock, AppError> {
    let duration_s = read_duration(node, "onDuration")?;
    let power_pct = read_power(node, "onPower")?;
    let cadence_rpm = read_cadence(node, "onCadence")?;

    Ok(WorkoutBlock::SteadyState {
        duration_s,
        power_pct,
        cadence_rpm: None,
        label: None,
    })
}

fn intervals_t_to_off_block(node: Node) -> Result<WorkoutBlock, AppError> {
    todo!()
}

fn steady_state_to_workout_block(steady_state_block: Node) -> Result<WorkoutBlock, AppError> {
    Ok(WorkoutBlock::SteadyState {
        duration_s: read_duration(steady_state_block, "duration")?,
        power_pct: read_power(steady_state_block, "power")?,
        cadence_rpm: read_cadence(steady_state_block, "cadence")?,
        label: read_label(steady_state_block),
    })
}

fn warmup_to_workout_block(warmup_node: Node) -> Result<WorkoutBlock, AppError> {
    Ok(WorkoutBlock::Ramp{
        duration_s: read_duration(warmup_node, "duration")?,
        power_start_pct: read_power(warmup_node, "powerlow")?,
        power_end_pct: read_power(warmup_node, "powerhigh")?,
        cadence_rpm: read_cadence(warmup_node, "cadence")?,
        label: read_label(warmup_node),
    })
}

fn read_duration(node: Node, attribute_name: &str) -> Result<u32, AppError> {
    node.attribute(attribute_name)
        .ok_or_else(|| AppError::ParseError("workout_block missing Duration".to_string()))?
        .parse::<u32>()
        .map_err(|e| AppError::ParseError(format!("invalid PowerLow: {e}")))
}

fn read_power(node: Node, attribute_name: &str) -> Result<f32, AppError> {
    node.attribute(attribute_name)
        .ok_or_else(|| AppError::ParseError("workout_block missing Power".to_string()))?
        .parse::<f32>()
        .map_err(|e| AppError::ParseError(format!("invalid Power: {e}")))
}

fn read_cadence(node: Node, attribute_name: &str) -> Result<Option<u16>, AppError> {
    match node.attribute(attribute_name) {
        Some(s) => s.parse::<u16>()
            .map_err(|e| AppError::ParseError(format!("invalid Cadence: {e}")))
            .map(Some),
        None => Ok(None),
    }
}
fn read_label(node: Node) -> Option<String> {
    match node.attribute("label") {
        Some(label) => Some(label.to_string()),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use crate::errors::AppError::ParseError;
    use super::*;

    const VALID_ZWO_FILE_PATH: &str = "tests/fixtures/test_workout.zwo";
    #[test]
    fn test_parse_zwo_given_valid_zwo_return_() {
        let file_content = fs::read_to_string(VALID_ZWO_FILE_PATH);
        assert!(file_content.is_ok());
        let parsed_workout = parse_zwo(&file_content.unwrap());
        assert!(parsed_workout.is_ok());
    }

    #[test]
    fn test_parse_zwo_given_empty_zwo_return_() {
        let parsed_workout = parse_zwo("");
        assert!(parsed_workout.is_err());
        assert!(matches!(parsed_workout, Err(ParseError(_))));
    }
}