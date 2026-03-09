//! Wire frame buffer module.

use serde::de::DeserializeOwned;

use crate::codec::try_decode_frame;
use crate::error::ProtocolError;

/// Incremental buffer for decoding length-prefixed protocol frames.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrameBuffer {
    /// Raw buffered bytes received from wire.
    bytes: Vec<u8>,
}

impl FrameBuffer {
    /// Default frame buffer constructor.
    #[must_use]
    pub const fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    /// Construct an empty frame buffer with a preallocated capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
        }
    }

    /**
     * Append a new chunk of transported bytes to buffer.
     *
     * # Arguments
     * - `chunk`: array of bytes to append.
     *
     * # Returns
     * `Ok(())` on succesfull byte insertion.
     *
     * # Errors
     * Returns `ProtocolError` if buffer would overflow.
     */
    pub fn append(&mut self, chunk: &[u8]) -> Result<(), ProtocolError> {
        let _new_len: usize = self
            .bytes
            .len()
            .checked_add(chunk.len())
            .ok_or(ProtocolError::CapacityOverflow)?;

        self.bytes.extend_from_slice(chunk);
        Ok(())
    }

    /**
     * Attempt to decode a single frame from the buffer.
     *
     * If a complete frame is available, it is decoded and then removed
     * from the internal buffer.
     *
     * If the buffer does not yet contain a full frame, no data is discarded from the buffer.
     *
     * # Returns
     * - `Ok(None)` - if no complete frame available in the buffer,
     * - decoded frame value - if a full frame is present in the buffer and succesfully decoded.
     *
     * # Errors
     * Returns `ProtocolError` if:
     * - buffer data contains a malformed complete frame,
     * - frame length is out of range,
     * - deserialization fails.
     */
    pub fn try_decode<T>(&mut self) -> Result<Option<T>, ProtocolError>
    where
        T: DeserializeOwned,
    {
        let Some((value, consumed)) = try_decode_frame::<T>(&self.bytes)? else {
            return Ok(None);
        };

        self.discard_prefix(consumed)?;
        Ok(Some(value))
    }

    /// Returns the number of currently buffered bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns `true` if buffer empty, `false` otherwise.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Clears buffer - removes all buffered bytes.
    pub fn clear(&mut self) {
        self.bytes.clear();
    }

    /// Returns a read-only view of buffered bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    /**
     * Discard a prefix of already-consumed bytes.
     *
     * # Arguments
     * - `count`: number of bytes to discard.
     *
     * # Returns
     * `Ok(())` on success.
     *
     * # Errors
     * Returns `ProtocolError` if `count` exceeds the current buffered length.
     */
    fn discard_prefix(&mut self, count: usize) -> Result<(), ProtocolError> {
        if count > self.len() {
            return Err(ProtocolError::InvalidDiscard {
                count,
                buffer_len: self.len(),
            });
        }

        if count == 0 {
            return Ok(());
        }

        if count == self.len() {
            self.bytes.clear();
            return Ok(());
        }

        self.bytes.copy_within(count.., 0);

        let new_len: usize = self
            .bytes
            .len()
            .checked_sub(count)
            .ok_or(ProtocolError::CapacityOverflow)?;

        self.bytes.truncate(new_len);
        Ok(())
    }
}

#[cfg(test)]
#[allow(dead_code, unused)]
mod tests {
    use hoy_test::assert_err;
    use serde::de::DeserializeOwned;

    use crate::codec::encode_frame;
    use crate::error::ProtocolError;
    use crate::frame_buffer::FrameBuffer;
    use crate::packet::ClientPacket;

    fn encode_ok(value: &impl serde::Serialize) -> Vec<u8> {
        encode_frame(value).expect("Frame encoding failed unexpectedly.")
    }

    fn append_ok(buffer: &mut FrameBuffer, chunk: &[u8]) {
        let len_og = buffer.len();
        buffer.append(chunk).expect("Append failed unexpectedly.");
        assert!(!buffer.is_empty());
        assert_eq!(
            buffer.len(),
            chunk
                .len()
                .checked_add(len_og)
                .expect("Unexpected buffer length overflow.")
        );
    }

    fn try_decode_ok<T>(buffer: &mut FrameBuffer) -> T
    where
        T: DeserializeOwned,
    {
        buffer
            .try_decode::<T>()
            .expect("Decoding failed unexpectedly.")
            .expect("Decoded frame should not be None.")
    }

    fn try_decode_none<T>(buffer: &mut FrameBuffer) -> Option<T>
    where
        T: DeserializeOwned + std::fmt::Debug + PartialEq,
    {
        let result = buffer
            .try_decode::<T>()
            .expect("Decoding failed unexpectedly.");
        assert_eq!(result, None);
        result
    }

    fn try_decode_err<T>(buffer: &mut FrameBuffer, error: &str) -> ProtocolError
    where
        T: DeserializeOwned + std::fmt::Debug,
    {
        buffer
            .try_decode::<T>()
            .expect_err(&format!("Decoding should return error: {error}"))
    }

    #[test]
    fn new_buffer_is_empty() {
        let buffer = FrameBuffer::new();

        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn append_adds_bytes_to_buffer() {
        let mut buffer = FrameBuffer::new();

        append_ok(&mut buffer, b"abcd");

        assert_eq!(buffer.len(), 4);
        assert_eq!(buffer.as_slice(), b"abcd");
    }

    #[test]
    fn try_decode_returns_none_for_incomplete_frame() {
        let mut buffer = FrameBuffer::new();

        buffer
            .append(&[0, 0, 0])
            .expect("Append failed unexpectedly.");

        let decoded = buffer
            .try_decode::<ClientPacket>()
            .expect("Decoding failed unexpectedly");
        assert_eq!(decoded, None);
        assert_eq!(buffer.as_slice(), &[0, 0, 0]);
    }

    #[test]
    fn try_decode_returns_complete_frame_and_consumes_it() {
        let mut buffer = FrameBuffer::new();
        let packet = ClientPacket::Ping;
        let frame = encode_ok(&packet);

        append_ok(&mut buffer, &frame);

        let decoded = try_decode_ok::<ClientPacket>(&mut buffer);
        assert_eq!(decoded, packet);
        assert!(buffer.is_empty());
    }

    #[test]
    fn try_decode_keeps_trailing_bytes_for_next_frame() {
        let mut buffer = FrameBuffer::new();

        let packet1 = ClientPacket::Ping;
        let packet2 = ClientPacket::Hello {
            username: String::from("bruce_lee"),
        };

        let frame1 = encode_ok(&packet1);
        let frame2 = encode_ok(&packet2);

        append_ok(&mut buffer, &frame1);
        append_ok(&mut buffer, &frame2);

        let decoded1 = try_decode_ok::<ClientPacket>(&mut buffer);
        assert_eq!(decoded1, packet1);
        assert!(!buffer.is_empty());
        assert_eq!(buffer.len(), frame2.len());

        let decoded2 = try_decode_ok::<ClientPacket>(&mut buffer);
        assert_eq!(decoded2, packet2);
        assert!(buffer.is_empty());
    }

    #[test]
    fn try_decode_returns_error_for_malformed_complete_frame() {
        let mut buffer = FrameBuffer::new();

        append_ok(&mut buffer, &[0, 0, 0, 4, b'b', b'a', b'd', b'!']);

        let error = try_decode_err::<ClientPacket>(&mut buffer, "Serde error");
        assert_err!(error, ProtocolError::Serde(_));
    }

    #[test]
    fn clear_removes_all_buffered_data() {
        let mut buffer = FrameBuffer::with_capacity(16);

        append_ok(&mut buffer, b"1234567890123456");

        buffer.clear();

        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }
}
