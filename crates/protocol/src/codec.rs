use serde::{de::DeserializeOwned, Serialize};

use crate::error::ProtocolError;

/// Protocol frame header length
const FRAME_HEADER_LEN: usize = 4;

/**
 * Encode a serializable value into a length-prefixed protocol frame.
 *
 * The wire format is:
 * - 4-byte big-endian payload length (`u32`)
 * - JSON payload bytes
 *
 * # Returns
 * `Ok(Vec<u8>)` on succesfull frame encoding.
 *
 * # Errors
 * Returns `ProtocolError` if:
 * - serialization fails,
 * - the serialized payload is too large to fit into a `u32` length prefix,
 * - the required output buffer capacity would overflow `usize`.
 */
pub fn encode_frame(value: &impl Serialize) -> Result<Vec<u8>, ProtocolError> {
    let payload: Vec<u8> = serde_json::to_vec(value)?;

    let payload_len_u32: u32 = u32::try_from(payload.len()).map_err(|e| {
        let _ = e;
        ProtocolError::FrameTooLarge {
            size: payload.len(),
        }
    })?;

    let frame_capacity: usize = FRAME_HEADER_LEN
        .checked_add(payload.len())
        .ok_or(ProtocolError::CapacityOverflow)?;

    let mut frame: Vec<u8> = Vec::with_capacity(frame_capacity);
    frame.extend_from_slice(&payload_len_u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    Ok(frame)
}

/**
 * Decode a length-prefixed protocol frame into a value.
 *
 * The input must contain a complete frame:
 * - 4-byte big-endian payload length (`u32`)
 * - exactly that many payload bytes
 *
 * Extra trailing bytes after the declared payload are ignored by this helper.
 * A streaming decoder can later handle multi-frame buffers more precisely.
 *
 * # Returns
 * `Ok(impl: DeserializeOwned)` decoded frame on success.
 *
 * # Errors
 * Returns an error if:
 * - the header is missing or malformed,
 * - the payload is truncated,
 * - the decoded frame length cannot be represented as `usize`,
 * - or JSON deserialization fails.
 */
pub fn decode_frame<T>(frame: &[u8]) -> Result<T, ProtocolError>
where
    T: DeserializeOwned,
{
    let header: &[u8] = frame
        .get(..FRAME_HEADER_LEN)
        .ok_or(ProtocolError::TruncatedFrame)?;

    let header_array: [u8; FRAME_HEADER_LEN] = <[u8; FRAME_HEADER_LEN]>::try_from(header)
        .map_err(|_e| ProtocolError::InvalidLengthPrefix)?;

    let payload_len_u32: u32 = u32::from_be_bytes(header_array);
    let payload_len =
        usize::try_from(payload_len_u32).map_err(|_e| ProtocolError::FrameLengthOutOfRange {
            length: payload_len_u32,
        })?;

    let payload_end: usize = FRAME_HEADER_LEN
        .checked_add(payload_len)
        .ok_or(ProtocolError::CapacityOverflow)?;
    let payload: &[u8] = frame
        .get(FRAME_HEADER_LEN..payload_end)
        .ok_or(ProtocolError::TruncatedFrame)?;

    let value = serde_json::from_slice(payload)?;
    Ok(value)
}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use serde::{de::DeserializeOwned, Serialize};

    use crate::{
        codec::{decode_frame, encode_frame},
        error::ProtocolError,
        packet::{ClientPacket, ServerPacket},
    };

    macro_rules! assert_err {
        ($value:expr, $error:pat) => {
            assert!(matches!($value, $error));
        };
    }

    fn build_frame(payload: &[u8]) -> Vec<u8> {
        let payload_len_u32 =
            u32::try_from(payload.len()).expect("test payload length capacity overflow");

        let frame_capacity: usize = 4_usize
            .checked_add(payload.len())
            .expect("test frame capacity overflow");

        let mut frame: Vec<u8> = Vec::with_capacity(frame_capacity);
        frame.extend_from_slice(&payload_len_u32.to_be_bytes());
        frame.extend_from_slice(payload);
        frame
    }

    fn encode_frame_ok(value: &impl Serialize) -> Vec<u8> {
        encode_frame(&value).expect("Frame encoding failed unexpectedly.")
    }

    fn encode_frame_err(value: &impl Serialize, error: &str) -> ProtocolError {
        encode_frame(&value).expect_err(&format!("Expected error: ${error}."))
    }

    fn decode_frame_ok<T>(frame: &[u8]) -> T
    where
        T: DeserializeOwned,
    {
        decode_frame(frame).expect("Frame deserialization failed unexpectedly.")
    }

    fn decode_frame_err<T>(frame: &[u8], error: &str) -> ProtocolError
    where
        T: DeserializeOwned + std::fmt::Debug,
    {
        decode_frame::<T>(frame).expect_err(&format!("Expected error: {error}."))
    }

    #[test]
    fn encode_and_decode_client_packet_roundtrip() {
        let packet = ClientPacket::Hello {
            username: String::from("bruce_lee"),
        };
        let frame = encode_frame_ok(&packet);
        let decoded: ClientPacket = decode_frame_ok(&frame);

        assert_eq!(decoded, packet);
    }

    #[test]
    fn encode_and_decode_server_packet_roundtrip() {
        let packet: ServerPacket = ServerPacket::ChatMesage {
            from: String::from("bruce_lee"),
            room: String::from("#general"),
            text: String::from("Kung foo..."),
        };
        let frame = encode_frame_ok(&packet);
        let decoded: ServerPacket = decode_frame_ok(&frame);

        assert_eq!(decoded, packet);
    }

    #[test]
    fn decode_frame_rejects_truncated_header() {
        let frame: Vec<u8> = vec![0, 0, 0];
        let error = decode_frame_err::<ClientPacket>(&frame, "Truncated header");

        assert_err!(error, ProtocolError::TruncatedFrame);
    }

    #[test]
    fn decode_frame_rejects_truncated_payload() {
        let declared_payload_len: u32 = 10;
        let mut frame: Vec<u8> = Vec::new();
        frame.extend_from_slice(&declared_payload_len.to_be_bytes());
        frame.extend_from_slice(b"abc");
        let error = decode_frame_err::<ClientPacket>(&frame, "Truncated payload");

        assert_err!(error, ProtocolError::TruncatedFrame);
    }

    #[test]
    fn decode_frame_rejects_invalid_json_payload() {
        let frame: Vec<u8> = build_frame(b"this is not valid json");
        let error = decode_frame_err::<ClientPacket>(&frame, "Serde error");

        assert_err!(error, ProtocolError::Serde(_));
    }

    #[test]
    fn decode_frame_rejects_json_of_wrong_packet_shape() {
        let payload: &[u8] = br#"{"NotARealPacket":{"foo":"bar"}}"#;
        let frame: Vec<u8> = build_frame(payload);
        let error = decode_frame_err::<ClientPacket>(&frame, "Serde error");

        assert_err!(error, ProtocolError::Serde(_));
    }

    #[test]
    fn decode_frame_ignores_trailing_bytes_after_payload() {
        let packet: ClientPacket = ClientPacket::Ping;
        let mut frame = encode_frame_ok(&packet);
        frame.extend_from_slice(b"trailing bytes that belong to a future frame");
        let decoded: ClientPacket = decode_frame_ok(&frame);

        assert_eq!(decoded, packet);
    }

    #[test]
    fn decode_frame_accepts_empty_string_fields() {
        let packet: ClientPacket = ClientPacket::Hello {
            username: String::new(),
        };
        let frame = encode_frame_ok(&packet);
        let decoded: ClientPacket = decode_frame_ok(&frame);

        assert_eq!(decoded, packet);
    }

    #[test]
    fn decode_frame_handles_utf8_content() {
        let packet: ServerPacket = ServerPacket::SystemMessage {
            text: String::from("Ahoj ^^ Привет こんにちは"),
        };
        let frame = encode_frame_ok(&packet);
        let decoded: ServerPacket = decode_frame_ok(&frame);

        assert_eq!(decoded, packet);
    }
}
