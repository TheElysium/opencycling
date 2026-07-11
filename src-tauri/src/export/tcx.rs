use crate::db::SessionDetail;
use crate::session::FlatBlock;
use chrono::{DateTime, Duration, FixedOffset};
use std::fmt::Write;

/// Formats start_time + offset seconds as RFC3339, preserving its offset.
fn point_time(start: &DateTime<FixedOffset>, offset_s: u32) -> String {
    (*start + Duration::seconds(offset_s as i64)).to_rfc3339()
}

/// Escapes XML text content. Only `&`, `<`, `>` are required in element text.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// One human-readable line per workout block (warmup / intervals / cooldown),
/// e.g. `Warmup: 10:00 @ 100-150 W`. Powers are absolute watts.
/// Reused as the Strava activity description (plain text, no XML escaping there).
pub fn workout_description(session: &SessionDetail) -> String {
    session
        .flat_blocks
        .iter()
        .map(|b| {
            let dur = format!("{}:{:02}", b.duration_s / 60, b.duration_s % 60);
            let power = describe_power(b);
            format!("{}: {dur} @ {power}", b.label)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn describe_power(b: &FlatBlock) -> String {
    if b.power_start_w == b.power_end_w {
        format!("{} W", b.power_start_w)
    } else {
        format!("{}-{} W", b.power_start_w, b.power_end_w)
    }
}

/// Builds a Garmin TCX activity (Sport="Biking") from a recorded session.
pub fn build_tcx(session: &SessionDetail) -> String {
    let mut out = String::with_capacity(256 + session.metrics.len() * 160);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(
        "<TrainingCenterDatabase \
xmlns=\"http://www.garmin.com/xmlschemas/TrainingCenterDatabase/v2\" \
xmlns:ns3=\"http://www.garmin.com/xmlschemas/ActivityExtension/v2\">\n",
    );
    out.push_str("  <Activities>\n");
    out.push_str("    <Activity Sport=\"Biking\">\n");
    writeln!(out, "      <Id>{}</Id>", session.started_at).unwrap();
    writeln!(out, "      <Lap StartTime=\"{}\">", session.started_at).unwrap();
    out.push_str("        <Track>\n");
    // Defensive: started_at is always valid RFC3339; a bad value yields an empty track.
    if let Ok(start) = DateTime::parse_from_rfc3339(&session.started_at) {
        for m in &session.metrics {
            out.push_str("          <Trackpoint>\n");
            writeln!(
                out,
                "            <Time>{}</Time>",
                point_time(&start, m.t_offset_s)
            )
            .unwrap();
            if let Some(hr) = m.hr_bpm {
                writeln!(
                    out,
                    "            <HeartRateBpm><Value>{hr}</Value></HeartRateBpm>"
                )
                .unwrap();
            }
            if let Some(cad) = m.cadence_rpm {
                writeln!(out, "            <Cadence>{cad}</Cadence>").unwrap();
            }
            if let Some(w) = m.power_w {
                writeln!(
                    out,
                    "            <Extensions><ns3:TPX><ns3:Watts>{w}</ns3:Watts></ns3:TPX></Extensions>"
                )
                .unwrap();
            }
            out.push_str("          </Trackpoint>\n");
        }
    }
    out.push_str("        </Track>\n");
    out.push_str("      </Lap>\n");
    if !session.flat_blocks.is_empty() {
        writeln!(
            out,
            "      <Notes>{}</Notes>",
            xml_escape(&workout_description(session))
        )
        .unwrap();
    }
    out.push_str("    </Activity>\n");
    out.push_str("  </Activities>\n");
    out.push_str("</TrainingCenterDatabase>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Metric, SessionDetail};

    fn base_session() -> SessionDetail {
        SessionDetail {
            id: 1,
            strava_activity_id: None,
            started_at: "2026-06-13T10:00:00+00:00".to_string(),
            ended_at: Some("2026-06-13T10:00:03+00:00".to_string()),
            workout_name: "Test Workout".to_string(),
            duration_s: Some(3),
            avg_power_w: Some(200),
            max_power_w: Some(250),
            avg_hr_bpm: Some(150),
            max_hr_bpm: Some(160),
            avg_cadence_rpm: Some(90),
            max_cadence_rpm: Some(95),
            ftp_w_used: 250,
            workout_type: None,
            aero_pct: None,
            np_w: None,
            if_: None,
            tss: None,
            flat_blocks: vec![],
            metrics: vec![],
        }
    }

    #[test]
    fn empty_session_is_valid_minimal_tcx() {
        let s = base_session();
        let xml = build_tcx(&s);
        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("<TrainingCenterDatabase"));
        assert!(xml.contains("Sport=\"Biking\""));
        assert!(xml.contains("<Id>2026-06-13T10:00:00+00:00</Id>"));
        assert!(xml.trim_end().ends_with("</TrainingCenterDatabase>"));
        assert!(!xml.contains("<Trackpoint>"));
        assert!(!xml.contains("<Notes>"));
    }

    #[test]
    fn notes_describe_blocks_and_escape_free_text() {
        use crate::session::FlatBlock;
        let mut s = base_session();
        s.flat_blocks = vec![
            FlatBlock {
                duration_s: 600,
                power_start_w: 100,
                power_end_w: 150,
                cadence_rpm: None,
                label: "Warmup".to_string(),
            },
            FlatBlock {
                duration_s: 240,
                power_start_w: 260,
                power_end_w: 260,
                cadence_rpm: Some(95),
                label: "Interval <hard> & fast".to_string(),
            },
        ];
        let xml = build_tcx(&s);

        assert!(xml.contains("<Notes>"));
        assert!(xml.contains("Warmup: 10:00 @ 100-150 W"));
        assert!(xml.contains("Interval &lt;hard&gt; &amp; fast: 4:00 @ 260 W"));
        assert!(!xml.contains("<hard>"));
    }

    #[test]
    fn trackpoints_render_time_and_present_fields_only() {
        let mut s = base_session();
        s.metrics = vec![
            Metric {
                t_offset_s: 0,
                power_w: Some(200),
                hr_bpm: Some(150),
                cadence_rpm: Some(90),
                aero_score: None,
            },
            Metric {
                t_offset_s: 1,
                power_w: None,
                hr_bpm: None,
                cadence_rpm: None,
                aero_score: None,
            },
        ];
        let xml = build_tcx(&s);

        assert!(xml.contains("<Time>2026-06-13T10:00:00+00:00</Time>"));
        assert!(xml.contains("<HeartRateBpm><Value>150</Value></HeartRateBpm>"));
        assert!(xml.contains("<Cadence>90</Cadence>"));
        assert!(xml.contains("<ns3:Watts>200</ns3:Watts>"));

        assert!(xml.contains("<Time>2026-06-13T10:00:01+00:00</Time>"));
        assert_eq!(xml.matches("<HeartRateBpm>").count(), 1);
        assert_eq!(xml.matches("<Cadence>").count(), 1);
        assert_eq!(xml.matches("<ns3:Watts>").count(), 1);
        assert_eq!(xml.matches("<Trackpoint>").count(), 2);
    }

    // Defensive branch at tcx.rs:57 -- a garbled started_at string must produce a
    // well-formed document with an empty track rather than panicking.
    #[test]
    fn invalid_started_at_yields_empty_track() {
        let mut s = base_session();
        s.started_at = "not-a-date".to_string();
        s.metrics = vec![Metric {
            t_offset_s: 0,
            power_w: Some(200),
            hr_bpm: None,
            cadence_rpm: None,
            aero_score: None,
        }];
        let xml = build_tcx(&s);
        // Document structure must be valid even with a bad timestamp.
        assert!(xml.contains("<TrainingCenterDatabase"));
        assert!(xml.trim_end().ends_with("</TrainingCenterDatabase>"));
        // No trackpoints should be emitted when the base time cannot be parsed.
        assert!(!xml.contains("<Trackpoint>"));
    }

    // Workout name containing XML-special characters must be escaped in <Notes>.
    // workout_description() formats the flat_blocks; the escaping happens in
    // build_tcx before writing <Notes>. This test also verifies that a workout
    // whose name would be unsafe is handled gracefully via the label path.
    #[test]
    fn xml_special_chars_in_block_label_are_escaped_in_notes() {
        use crate::session::FlatBlock;
        let mut s = base_session();
        s.flat_blocks = vec![FlatBlock {
            duration_s: 60,
            power_start_w: 200,
            power_end_w: 200,
            cadence_rpm: None,
            label: "Zone 3 & <Tempo>".to_string(),
        }];
        let xml = build_tcx(&s);
        assert!(xml.contains("Zone 3 &amp; &lt;Tempo&gt;"));
        // Raw unescaped forms must not appear inside the Notes element.
        assert!(!xml.contains("<Tempo>"));
    }

    // A session with metrics but no HR or cadence must not emit those elements.
    #[test]
    fn power_only_sample_omits_hr_and_cadence_elements() {
        let mut s = base_session();
        s.metrics = vec![Metric {
            t_offset_s: 0,
            power_w: Some(150),
            hr_bpm: None,
            cadence_rpm: None,
            aero_score: None,
        }];
        let xml = build_tcx(&s);
        assert!(xml.contains("<ns3:Watts>150</ns3:Watts>"));
        assert!(!xml.contains("<HeartRateBpm>"));
        assert!(!xml.contains("<Cadence>"));
    }
}
