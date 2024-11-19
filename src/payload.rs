use bytes::{Bytes, BytesMut};
use rand::Rng;

/// A trait for payloads that can be used with the TrafficShaper.
pub trait TrafficShaperPayload: Unpin + Send + Sync + 'static {
    fn byte_len(&self) -> usize;

    fn corrupt(&mut self);
}

impl TrafficShaperPayload for Bytes {
    fn byte_len(&self) -> usize {
        Bytes::len(self)
    }

    fn corrupt(&mut self) {
        let index = rand::thread_rng().gen_range(0..self.len());
        let mut packet_modified = BytesMut::from(self.to_owned());
        packet_modified[index] ^= 0xFF; // Corrupt one byte
        *self = packet_modified.freeze();
    }
}
