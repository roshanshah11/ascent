//! Framing codec verification: partial frames, concatenation, and every
//! fail-closed case (zero length, oversize, bad UTF-8, bad JSON, unknown
//! version, and poisoning after a violation).

use ascent_visualizer_protocol::{
    encode_frame, ClientRequest, CodecError, Envelope, FrameDecoder, MAX_FRAME_BYTES,
    PROTOCOL_VERSION,
};

fn hello_fixture() -> Envelope<ClientRequest> {
    Envelope::new(
        "msg-1",
        None,
        ClientRequest::Hello {
            client: "unity".into(),
            protocol_version: PROTOCOL_VERSION,
            capabilities: Vec::new(),
        },
    )
}

fn list_fixture() -> Envelope<ClientRequest> {
    Envelope::new("msg-2", Some("req-1".into()), ClientRequest::ListMissions)
}

#[test]
fn decoder_retains_partial_frames_and_emits_concatenated_frames() {
    let first = encode_frame(&hello_fixture()).unwrap();
    let second = encode_frame(&list_fixture()).unwrap();
    let mut decoder = FrameDecoder::new(MAX_FRAME_BYTES);
    assert!(decoder.push(&first[..3]).unwrap().is_empty());
    let mut rest = first[3..].to_vec();
    rest.extend_from_slice(&second);
    let decoded = decoder.push(&rest).unwrap();
    assert_eq!(decoded, vec![hello_fixture(), list_fixture()]);
}

#[test]
fn decoder_reassembles_byte_at_a_time() {
    let frame = encode_frame(&hello_fixture()).unwrap();
    let mut decoder = FrameDecoder::<ClientRequest>::new(MAX_FRAME_BYTES);
    for (i, byte) in frame.iter().enumerate() {
        let decoded = decoder.push(&[*byte]).unwrap();
        if i + 1 == frame.len() {
            assert_eq!(decoded, vec![hello_fixture()]);
        } else {
            assert!(decoded.is_empty());
        }
    }
}

#[test]
fn zero_length_frame_is_rejected_and_poisons() {
    let mut decoder = FrameDecoder::<ClientRequest>::new(MAX_FRAME_BYTES);
    let err = decoder.push(&0u32.to_le_bytes()).unwrap_err();
    assert!(matches!(err, CodecError::EmptyFrame));
    assert!(matches!(
        decoder.push(b"anything").unwrap_err(),
        CodecError::Poisoned
    ));
}

#[test]
fn oversize_frame_is_rejected_before_buffering_the_body() {
    let mut decoder = FrameDecoder::<ClientRequest>::new(MAX_FRAME_BYTES);
    let len = (MAX_FRAME_BYTES + 1) as u32;
    let err = decoder.push(&len.to_le_bytes()).unwrap_err();
    assert!(matches!(err, CodecError::FrameTooLarge { .. }));
    assert!(matches!(
        decoder.push(b"x").unwrap_err(),
        CodecError::Poisoned
    ));
}

#[test]
fn malformed_utf8_is_rejected() {
    let body = [0xff, 0xfe, 0xfd];
    let mut framed = (body.len() as u32).to_le_bytes().to_vec();
    framed.extend_from_slice(&body);
    let mut decoder = FrameDecoder::<ClientRequest>::new(MAX_FRAME_BYTES);
    assert!(matches!(
        decoder.push(&framed).unwrap_err(),
        CodecError::Utf8
    ));
}

#[test]
fn malformed_json_is_rejected() {
    let body = b"{not json";
    let mut framed = (body.len() as u32).to_le_bytes().to_vec();
    framed.extend_from_slice(body);
    let mut decoder = FrameDecoder::<ClientRequest>::new(MAX_FRAME_BYTES);
    assert!(matches!(
        decoder.push(&framed).unwrap_err(),
        CodecError::Json(_)
    ));
}

#[test]
fn unknown_protocol_version_is_rejected() {
    let mut envelope = hello_fixture();
    envelope.protocol_version = 999;
    let frame = encode_frame(&envelope).unwrap();
    let mut decoder = FrameDecoder::<ClientRequest>::new(MAX_FRAME_BYTES);
    assert!(matches!(
        decoder.push(&frame).unwrap_err(),
        CodecError::ProtocolMismatch {
            found: 999,
            expected: 1
        }
    ));
}
