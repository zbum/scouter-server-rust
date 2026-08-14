use scouter_server_rust::protocol::data_input::DataInputX;
use scouter_server_rust::protocol::data_output::DataOutputX;
use scouter_server_rust::protocol::pack::Pack;
use scouter_server_rust::protocol::value::Value;

const JAVA_VALUES_HEX: &str = "000a01140880000000000000001ec148000028400921fb54442d182d4010000000000000000000023ff800000000000040040000000000002efffffffffffffff700000003fffffffffffffff90000000000000004321353636f7574657220ed959ceab88020f09f9a8033123456783c030001ff3d7f00000146010314010732046c6973740a0047000380000000000000007fffffff480002800000007f7fffff4900020006eab2bdeab3844a0003800000000000000000000000000000007fffffffffffffff500101046f6e6c79320576616c7565";
const JAVA_TEXT_PACK_HEX: &str = "320753455256494345f8a432eb0c5554462d3820ed959ceab880";

fn decode_hex(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(text, 16).unwrap()
        })
        .collect()
}

#[test]
fn java_value_golden_stream_decodes_and_reencodes_identically() {
    let golden = decode_hex(JAVA_VALUES_HEX);
    let mut input = DataInputX::from_bytes(golden.clone());
    let mut values = Vec::new();
    while input.available() > 0 {
        let index = values.len();
        let offset = input.offset();
        values.push(
            input
                .read_value()
                .unwrap_or_else(|error| panic!("value {index} at offset {offset}: {error}")),
        );
    }

    assert_eq!(values.len(), 17);
    assert!(matches!(values[0], Value::Null));
    assert!(matches!(values[1], Value::Boolean(true)));
    assert!(matches!(values[2], Value::Decimal(i64::MIN)));
    assert!(matches!(&values[7], Value::Text(text) if text == "Scouter 한글 🚀"));
    assert!(matches!(&values[11], Value::List(items) if items.len() == 3));
    assert!(matches!(&values[16], Value::Map(map) if map.get_text("only") == Some("value")));

    let mut encoded = DataOutputX::new();
    for value in &values {
        encoded.write_value(value).unwrap();
    }
    assert_eq!(
        encoded.to_bytes(),
        golden,
        "Rust encoder diverged from Java golden bytes"
    );
}

#[test]
fn java_text_pack_golden_decodes_and_reencodes_identically() {
    let golden = decode_hex(JAVA_TEXT_PACK_HEX);
    let mut input = DataInputX::from_bytes(golden.clone());
    let pack = input.read_pack().unwrap();

    match &pack {
        Pack::Text(text) => {
            assert_eq!(text.xtype, "SERVICE");
            assert_eq!(text.hash, -123456789);
            assert_eq!(text.text, "UTF-8 한글");
        }
        other => panic!("expected TextPack, got {other}"),
    }

    let mut encoded = DataOutputX::new();
    encoded.write_pack(&pack).unwrap();
    assert_eq!(
        encoded.to_bytes(),
        golden,
        "Rust encoder diverged from Java golden bytes"
    );
}

#[test]
fn unknown_value_and_pack_types_are_rejected() {
    let mut value = DataInputX::from_bytes(vec![0xff]);
    assert!(value.read_value().is_err());

    let mut pack = DataInputX::from_bytes(vec![0xff]);
    assert!(pack.read_pack().is_err());
}

#[test]
fn negative_collection_length_is_rejected_without_allocation() {
    // ListValue followed by decimal -1 (length marker 1, value ff).
    let mut input = DataInputX::from_bytes(vec![70, 1, 0xff]);
    assert!(input.read_value().is_err());
}
