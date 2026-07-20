use std::collections::BTreeMap;

use ascent_domain::evidence::{ChannelKind, TelemetryBundle, TrackKind, Unit};
use ascent_domain::telemetry::{
    builtin_schema, BinaryField, BinaryValueType, BuiltinSchema, ChannelMapping, FrameSchema,
    ImportFormat, ImportSchema, SourceSchema, TelemetryImporter, TimeMapping,
};

#[test]
fn schema_driven_csv_preserves_raw_bytes_and_normalizes_explicit_si_channels() {
    let schema = ImportSchema::from_json(
        br#"{
          "schema_version":1,
          "schema_id":"professional-imu-v1",
          "format":{"kind":"csv","delimiter":",","has_header":true},
          "source":{"source_id":"imu-a","device":"flight-computer","firmware":"2.4","export_format":"engineering-csv"},
          "time":{"field":"device_us","scale_to_seconds":0.000001,"domain":"fc-monotonic","system":"device_elapsed"},
          "frame":{"id":"imu-body","kind":"sensor_body"},
          "channels":[
            {"fields":["ax_g","ay_g","az_g"],"id":"imu.specific_force","kind":"specific_force","track":"raw","unit":"meter_per_second_squared","scale":9.80665,"offset":0.0,"uncertainty":0.02},
            {"fields":["valid"],"id":"imu.valid","kind":"scalar","track":"raw","unit":"dimensionless","scale":1.0,"offset":0.0}
          ]
        }"#,
    )
    .unwrap();
    let raw = b"device_us,ax_g,ay_g,az_g,valid\n1000000,1.0,0.0,0.0,1\n1010000,1.1,0.1,0.0,1\n";
    let bundle = TelemetryImporter::ingest(&schema, raw).unwrap();

    assert_eq!(bundle.sources[0].bytes, raw);
    assert_eq!(bundle.sources[0].sha256.len(), 64);
    assert_eq!(bundle.trace.channels[0].kind, ChannelKind::SpecificForce);
    assert_eq!(bundle.trace.channels[0].track, TrackKind::Raw);
    assert_eq!(bundle.trace.channels[0].unit, Unit::MeterPerSecondSquared);
    assert_eq!(bundle.trace.channels[0].samples[0].time, 1.0);
    assert!((bundle.trace.channels[0].samples[0].values[0] - 9.80665).abs() < 1e-12);
    bundle.validate().unwrap();
}

#[test]
fn ambiguous_or_missing_columns_fail_instead_of_becoming_canonical_evidence() {
    let mut schema = builtin_schema(BuiltinSchema::PerfectFliteCsv);
    schema.channels[0].unit = None;
    assert!(TelemetryImporter::ingest(&schema, b"Time,Altitude\n0,0\n")
        .unwrap_err()
        .contains("unit"));

    let schema = builtin_schema(BuiltinSchema::Rrc3Csv);
    assert!(TelemetryImporter::ingest(&schema, b"wrong,columns\n0,0\n")
        .unwrap_err()
        .contains("missing column"));
}

#[test]
fn consumer_formats_are_named_edge_adapters_not_the_canonical_model() {
    for schema in [
        builtin_schema(BuiltinSchema::PerfectFliteCsv),
        builtin_schema(BuiltinSchema::Rrc3Csv),
        builtin_schema(BuiltinSchema::BlueRavenCsv),
    ] {
        assert!(schema.schema_id.starts_with("edge."));
        assert!(schema.channels.iter().all(|channel| channel.unit.is_some()));
    }
}

#[test]
fn json_lines_and_binary_sources_merge_without_collapsing_clock_domains() {
    let json_schema = ImportSchema::from_json(
        br#"{"schema_version":1,"schema_id":"gnss-json","format":{"kind":"json_lines"},"source":{"source_id":"gnss","device":"GNSS receiver","firmware":null,"export_format":"jsonl"},"time":{"field":"gps_s","scale_to_seconds":1.0,"domain":"gps","system":"gps"},"frame":{"id":"ecef","kind":"earth_centered_earth_fixed"},"channels":[{"fields":["x","y","z"],"id":"gnss.position","kind":"position","track":"raw","unit":"meter","scale":1.0,"offset":0.0,"uncertainty":2.0}]}"#,
    )
    .unwrap();
    let gnss = TelemetryImporter::ingest(
        &json_schema,
        b"{\"gps_s\":100.0,\"x\":1.0,\"y\":2.0,\"z\":3.0}\n{\"gps_s\":100.1,\"x\":1.1,\"y\":2.1,\"z\":3.1}\n",
    )
    .unwrap();

    let binary_schema = ImportSchema {
        schema_version: 1,
        schema_id: "actuator-binary".into(),
        format: ImportFormat::Binary {
            record_size: 12,
            fields: BTreeMap::from([
                (
                    "ticks".into(),
                    BinaryField {
                        offset: 0,
                        value_type: BinaryValueType::U32Le,
                    },
                ),
                (
                    "command".into(),
                    BinaryField {
                        offset: 4,
                        value_type: BinaryValueType::F64Le,
                    },
                ),
            ]),
        },
        source: SourceSchema {
            source_id: "actuator".into(),
            device: "flight actuator controller".into(),
            firmware: Some("1.0".into()),
            export_format: "fixed record".into(),
        },
        time: TimeMapping {
            field: "ticks".into(),
            scale_to_seconds: 0.001,
            domain: "actuator-clock".into(),
            system: ascent_domain::evidence::TimeSystem::DeviceElapsed,
        },
        frame: FrameSchema {
            id: "vehicle-body".into(),
            kind: ascent_domain::evidence::FrameKind::VehicleBody,
        },
        channels: vec![ChannelMapping {
            fields: vec!["command".into()],
            id: "actuator.command".into(),
            kind: ChannelKind::ActuatorState,
            track: TrackKind::Raw,
            unit: Some(Unit::Dimensionless),
            scale: 1.0,
            offset: 0.0,
            uncertainty: None,
            valid_field: None,
        }],
    };
    let mut raw = Vec::new();
    raw.extend_from_slice(&1_000_u32.to_le_bytes());
    raw.extend_from_slice(&0.25_f64.to_le_bytes());
    raw.extend_from_slice(&1_100_u32.to_le_bytes());
    raw.extend_from_slice(&0.50_f64.to_le_bytes());
    let actuator = TelemetryImporter::ingest(&binary_schema, &raw).unwrap();

    let merged = TelemetryBundle::merge("mission-avionics", vec![gnss, actuator]).unwrap();
    assert_eq!(merged.sources.len(), 2);
    assert_eq!(merged.trace.time_bases.len(), 2);
    assert_eq!(merged.trace.channels.len(), 2);
    assert_eq!(merged.trace.channels[1].samples[0].time, 1.0);
    merged.validate().unwrap();
}
