use bytes::{
    Bytes,
    BytesMut,
};
use rand::Rng;

/// A trait for payloads that can be used with the TrafficShaper.
pub trait ChokeItem: Unpin + Sized + 'static {
    fn byte_len(&self) -> usize;

    fn corrupt(&mut self);

    fn duplicate(&mut self) -> Option<Self> {
        None
    }
}

impl ChokeItem for Bytes {
    fn byte_len(&self) -> usize {
        Bytes::len(self)
    }

    fn corrupt(&mut self) {
        let index = rand::rng().random_range(0..self.len());
        let mut packet_modified = BytesMut::from(self.to_owned());
        packet_modified[index] ^= 0xFF; // Corrupt one byte
        *self = packet_modified.freeze();
    }

    fn duplicate(&mut self) -> Option<Self> {
        Some(self.clone())
    }
}

impl<T, E> ChokeItem for Result<T, E>
where
    T: ChokeItem,
    E: Unpin + Send + Sync + 'static,
{
    fn byte_len(&self) -> usize {
        self.as_ref().map_or(0, |payload| payload.byte_len())
    }

    fn corrupt(&mut self) {
        if let Ok(payload) = self {
            payload.corrupt();
        }
    }

    fn duplicate(&mut self) -> Option<Self> {
        self.as_mut().ok().and_then(|payload| payload.duplicate().map(Ok))
    }
}

impl<T> ChokeItem for Option<T>
where
    T: ChokeItem,
{
    fn byte_len(&self) -> usize {
        self.as_ref().map_or(0, |payload| payload.byte_len())
    }

    fn corrupt(&mut self) {
        if let Some(payload) = self {
            payload.corrupt();
        }
    }

    fn duplicate(&mut self) -> Option<Self> {
        self.as_mut().and_then(|payload| payload.duplicate().map(Some))
    }
}
