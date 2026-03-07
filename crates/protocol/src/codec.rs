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
