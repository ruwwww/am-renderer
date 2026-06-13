//! XML file reader for Alight Motion project files.
//!
//! The single entry point is [`parse_xml`], which reads an `.xml` file from
//! disk and deserialises it into an [`XmlScene`].

use std::path::Path;

use anyhow::{Context, Result};

use super::types::XmlScene;

/// Parse an Alight Motion XML project file into the raw [`XmlScene`] type.
///
/// # Errors
///
/// Returns an error if the file cannot be read or if the XML structure does
/// not match the expected Alight Motion format.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use alight_parser::parse_xml;
///
/// let scene = parse_xml(Path::new("project.xml")).unwrap();
/// println!("Scene size: {}x{}", scene.width, scene.height);
/// ```
pub fn parse_xml(path: &Path) -> Result<XmlScene> {
    let xml_str = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read XML file: {}", path.display()))?;

    let scene: XmlScene = quick_xml::de::from_str(&xml_str)
        .with_context(|| format!("failed to deserialise XML from: {}", path.display()))?;

    log::info!(
        "Parsed scene: {}x{}, {} media, {} shapes, {} audio layers",
        scene.width,
        scene.height,
        scene.media().len(),
        scene.shapes().len(),
        scene.audio().len(),
    );

    Ok(scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: deserialise a minimal valid scene from an in-memory string.
    #[test]
    fn parse_minimal_scene() {
        let xml = r#"<?xml version='1.0' encoding='UTF-8' ?>
<scene width="1080" height="1920" totalTime="5000" fps="30">
</scene>"#;

        let scene: XmlScene = quick_xml::de::from_str(xml).expect("failed to parse minimal scene");
        assert_eq!(scene.width, "1080");
        assert_eq!(scene.height, "1920");
        assert_eq!(scene.total_time, "5000");
        assert_eq!(scene.fps, "30");
        assert!(scene.shapes().is_empty());
        assert!(scene.audio().is_empty());
        assert!(scene.media().is_empty());
    }

    /// Test parsing a scene with media, shapes, transforms, keyframes, effects,
    /// and audio – the full structure from a real export.
    #[test]
    fn parse_full_scene() {
        let xml = r##"<?xml version='1.0' encoding='UTF-8' ?>
<scene title="Test" width="1080" height="1920" exportWidth="1080" exportHeight="1920"
       bgcolor="#FF000000" totalTime="13300" fps="60" amver="737" ffver="107"
       am="com.alightcreative.motion/6.2.6" amplatform="ios"
       precompose="dynamicResolution" retime="freeze">
  <media uri="am-internal:///abc.mp3" filename="song.mp3" title="Song"
         type="audio/mpeg" duration="29235000" size="1169408" sig="HASH123"/>
  <media uri="am-internal:///def.PNG" type="image/png" size="939837"
         width="540" height="960"/>
  <bookmark t="5416"/>
  <audio id="410972" label="Audio" startTime="0" endTime="5432"
         src="am-internal:///abc.mp3" inTime="18366">
    <gain>
      <kf t="0.000000" v="0.000000"/>
      <kf t="0.978645" v="0.000000" e="local cubicBezier 0.0 0.0 0.58 1.0"/>
    </gain>
  </audio>
  <shape id="410976" label="image.jpg" startTime="0" endTime="5432"
         fillType="media" fillImage="am-internal:///def.PNG"
         mediaFillMode="stretch" s=".rect" blending="multiply" hidden="true">
    <transform>
      <location value="540.000000,960.000000,0.000000"/>
      <scale>
        <kf t="0.002946" v="1.522917,1.522917"/>
        <kf t="0.481591" v="1.000000,1.000000" e="local cubicBezier 0.0 0.0 0.58 1.0"/>
      </scale>
      <rotation>
        <kf t="0.022890" v="360.000000"/>
        <kf t="0.977110" v="0.000000" e="local cubicBezier 0.0 0.0 0.0 1.0"/>
      </rotation>
      <opacity>
        <kf t="0.917342" v="1.000000"/>
        <kf t="0.997055" v="0.000000" e="local cubicBezier 0.42 0.0 1.0 1.0"/>
      </opacity>
    </transform>
    <effect id="com.alightcreative.effects.tile" locallyApplied="true">
      <property name="mirror" type="bool" value="true"/>
    </effect>
    <effect id="com.alightcreative.effects.oscillate3" locallyApplied="true">
      <property name="freq" type="float">
        <kf t="0.002243" v="3.350000"/>
        <kf t="0.997757" v="0.380000" e="local cubicBezier 0.0 0.80374336 0.58 1.0"/>
      </property>
      <property name="mag" type="float">
        <kf t="0.002243" v="357.000000"/>
        <kf t="0.997757" v="10.000000" e="local cubicBezier 0.0 1.0 0.23572138 1.0"/>
      </property>
    </effect>
    <effect id="com.alightcreative.effects.motionblur3" locallyApplied="true"/>
    <fillColor value="#FF000000"/>
    <gradient type="linear" startColor="#FF000000" endColor="#FFFFFFFF"
              start="0.416472,0.208469" end="1.000000,1.000000"/>
    <property name="size" type="vec2" value="540.000000,960.000000"/>
  </shape>
</scene>"##;

        let scene: XmlScene = quick_xml::de::from_str(xml).expect("failed to parse full scene");

        // Scene-level
        assert_eq!(scene.title.as_deref(), Some("Test"));
        assert_eq!(scene.width, "1080");
        assert_eq!(scene.height, "1920");
        assert_eq!(scene.fps, "60");
        assert_eq!(scene.total_time, "13300");
        assert_eq!(scene.bgcolor.as_deref(), Some("#FF000000"));
        assert_eq!(scene.amplatform.as_deref(), Some("ios"));
        assert_eq!(scene.retime.as_deref(), Some("freeze"));

        // Media
        assert_eq!(scene.media().len(), 2);
        assert_eq!(scene.media()[0].uri, "am-internal:///abc.mp3");
        assert_eq!(scene.media()[0].r#type.as_deref(), Some("audio/mpeg"));
        assert_eq!(scene.media()[1].width.as_deref(), Some("540"));

        // Bookmarks
        assert_eq!(scene.bookmarks().len(), 1);
        assert_eq!(scene.bookmarks()[0].t, "5416");

        // Audio
        assert_eq!(scene.audio().len(), 1);
        let audio = scene.audio()[0];
        assert_eq!(audio.id, "410972");
        assert_eq!(audio.start_time, "0");
        assert_eq!(audio.end_time, "5432");
        assert_eq!(audio.src.as_deref(), Some("am-internal:///abc.mp3"));
        assert_eq!(audio.in_time.as_deref(), Some("18366"));
        assert_eq!(audio.out_time, None);
        let gain = audio.gain.as_ref().expect("gain missing");
        assert_eq!(gain.keyframes.len(), 2);
        assert_eq!(
            gain.keyframes[1].e.as_deref(),
            Some("local cubicBezier 0.0 0.0 0.58 1.0")
        );

        // Shape
        assert_eq!(scene.shapes().len(), 1);
        let shape = scene.shapes()[0];
        assert_eq!(shape.id, "410976");
        assert_eq!(shape.fill_type.as_deref(), Some("media"));
        assert_eq!(shape.blending.as_deref(), Some("multiply"));
        assert_eq!(shape.hidden.as_deref(), Some("true"));
        assert_eq!(shape.s.as_deref(), Some(".rect"));

        // Transform – static location
        let transform = shape.transform.as_ref().expect("transform missing");
        let loc = transform.location.as_ref().expect("location missing");
        assert_eq!(loc.value.as_deref(), Some("540.000000,960.000000,0.000000"));
        assert!(loc.keyframes.is_empty());

        // Transform – animated scale
        let scale = transform.scale.as_ref().expect("scale missing");
        assert!(scale.value.is_none());
        assert_eq!(scale.keyframes.len(), 2);
        assert_eq!(scale.keyframes[0].v, "1.522917,1.522917");

        // Transform – animated rotation
        let rot = transform.rotation.as_ref().expect("rotation missing");
        assert_eq!(rot.keyframes.len(), 2);

        // Transform – animated opacity
        let opa = transform.opacity.as_ref().expect("opacity missing");
        assert_eq!(opa.keyframes.len(), 2);

        // Effects
        assert_eq!(shape.effects.len(), 3);
        // Tile effect with static property
        assert_eq!(shape.effects[0].id, "com.alightcreative.effects.tile");
        assert_eq!(shape.effects[0].properties.len(), 1);
        assert_eq!(shape.effects[0].properties[0].name, "mirror");
        assert_eq!(
            shape.effects[0].properties[0].value.as_deref(),
            Some("true")
        );

        // Oscillate effect with animated properties
        assert_eq!(shape.effects[1].id, "com.alightcreative.effects.oscillate3");
        assert_eq!(shape.effects[1].properties.len(), 2);
        assert_eq!(shape.effects[1].properties[0].name, "freq");
        assert_eq!(shape.effects[1].properties[0].keyframes.len(), 2);

        // Motion blur effect with no properties
        assert_eq!(
            shape.effects[2].id,
            "com.alightcreative.effects.motionblur3"
        );
        assert!(shape.effects[2].properties.is_empty());

        // Fill colour
        let fc = shape.fill_color.as_ref().expect("fillColor missing");
        assert_eq!(fc.value, "#FF000000");

        // Gradient
        let grad = shape.gradient.as_ref().expect("gradient missing");
        assert_eq!(grad.r#type.as_deref(), Some("linear"));
        assert_eq!(grad.start_color.as_deref(), Some("#FF000000"));
        assert_eq!(grad.end_color.as_deref(), Some("#FFFFFFFF"));

        // Top-level shape property
        assert_eq!(shape.properties.len(), 1);
        assert_eq!(shape.properties[0].name, "size");
        assert_eq!(
            shape.properties[0].value.as_deref(),
            Some("540.000000,960.000000")
        );
    }

    /// Ensure static opacity/rotation (value attr, no keyframes) parses.
    #[test]
    fn parse_static_transform_children() {
        let xml = r#"<?xml version='1.0' encoding='UTF-8' ?>
<scene width="100" height="100" totalTime="1000" fps="30">
  <shape id="1" startTime="0" endTime="1000">
    <transform>
      <location value="50.0,50.0,0.0"/>
      <scale value="1.0,1.0"/>
      <rotation value="45.0"/>
      <opacity value="0.5"/>
    </transform>
  </shape>
</scene>"#;

        let scene: XmlScene = quick_xml::de::from_str(xml).expect("parse failed");
        let t = scene.shapes()[0].transform.as_ref().unwrap();

        assert_eq!(
            t.location.as_ref().unwrap().value.as_deref(),
            Some("50.0,50.0,0.0")
        );
        assert_eq!(t.scale.as_ref().unwrap().value.as_deref(), Some("1.0,1.0"));
        assert_eq!(t.rotation.as_ref().unwrap().value.as_deref(), Some("45.0"));
        assert_eq!(t.opacity.as_ref().unwrap().value.as_deref(), Some("0.5"));
    }

    /// Multiple shapes, audio layers, media and bookmarks.
    #[test]
    fn parse_multiple_layers() {
        let xml = r#"<?xml version='1.0' encoding='UTF-8' ?>
<scene width="1080" height="1920" totalTime="10000" fps="60">
  <media uri="am-internal:///a.png" type="image/png"/>
  <media uri="am-internal:///b.mp3" type="audio/mpeg"/>
  <bookmark t="1000"/>
  <bookmark t="5000"/>
  <audio id="1" startTime="0" endTime="10000"/>
  <audio id="2" startTime="500" endTime="9000"/>
  <shape id="10" startTime="0" endTime="5000"/>
  <shape id="20" startTime="2000" endTime="8000" blending="screen"/>
  <shape id="30" startTime="0" endTime="10000" hidden="true"/>
</scene>"#;

        let scene: XmlScene = quick_xml::de::from_str(xml).expect("parse failed");
        assert_eq!(scene.media().len(), 2);
        assert_eq!(scene.bookmarks().len(), 2);
        assert_eq!(scene.audio().len(), 2);
        assert_eq!(scene.shapes().len(), 3);
        assert_eq!(scene.shapes()[1].blending.as_deref(), Some("screen"));
        assert_eq!(scene.shapes()[2].hidden.as_deref(), Some("true"));
    }
}
