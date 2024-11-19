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

impl<T, E> TrafficShaperPayload for Result<T, E>
where
    T: TrafficShaperPayload,
    E: Unpin + Send + Sync + 'static,
{
    fn byte_len(&self) -> usize {
        match self {
            Ok(payload) => payload.byte_len(),
            Err(_) => 0,
        }
    }

    fn corrupt(&mut self) {
        if let Ok(payload) = self {
            payload.corrupt();
        }
    }
}

impl<T> TrafficShaperPayload for Option<T>
where
    T: TrafficShaperPayload,
{
    fn byte_len(&self) -> usize {
        match self {
            Some(payload) => payload.byte_len(),
            None => 0,
        }
    }

    fn corrupt(&mut self) {
        if let Some(payload) = self {
            payload.corrupt();
        }
    }
}
