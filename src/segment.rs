#[cfg(test)]
mod tests;

use self::Kind::*;
use rand::distributions::{Distribution, Standard};
use rand::Rng;

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Segment {
    kind: Kind,
    seq_num: u32,
    ack_num: u32,
    data: Vec<u8>,
}

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum Kind {
    Syn,
    SynAck,
    Ack,
    Fin,
    // TODO: FinAck?
}

impl Distribution<Kind> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Kind {
        match rng.next_u32() % 4 {
            0 => Syn,
            1 => SynAck,
            2 => Ack,
            3 => Fin,
            _ => unimplemented!(),
        }
    }
}

impl Segment {
    pub fn new_empty(kind: Kind, seq_num: u32, ack_num: u32) -> Segment {
        Self::new(kind, seq_num, ack_num, &[])
    }

    pub fn new(kind: Kind, seq_num: u32, ack_num: u32, data: &[u8]) -> Segment {
        Segment {
            kind,
            seq_num,
            ack_num,
            data: data.to_vec(),
        }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn seq_num(&self) -> u32 {
        self.seq_num
    }

    pub fn ack_num(&self) -> u32 {
        self.ack_num
    }

    pub fn set_ack_num(&self, ack_num: u32) -> Self {
        let mut new_seg = self.clone();
        new_seg.ack_num = ack_num;
        new_seg
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::new();

        let syn = self.kind == Syn || self.kind == SynAck;
        let ack = self.kind == SynAck || self.kind == Ack;
        let fin = self.kind == Fin;

        let mut first_byte: u8 = 0;
        first_byte |= (syn as u8) << 7;
        first_byte |= (ack as u8) << 6;
        first_byte |= (fin as u8) << 5;
        encoded.push(first_byte);

        encoded.extend_from_slice(&self.seq_num.to_be_bytes());
        encoded.extend_from_slice(&self.ack_num.to_be_bytes());
        encoded.extend_from_slice(&self.data);

        encoded
    }

    pub fn decode(raw_data: &[u8]) -> Option<Segment> {
        if raw_data.len() >= 9 {
            let first_byte = raw_data[0];
            let syn = (first_byte & 0b1000_0000) != 0;
            let ack = (first_byte & 0b0100_0000) != 0;
            let fin = (first_byte & 0b0010_0000) != 0;

            let maybe_kind = match (syn, ack, fin) {
                (true, false, false) => Some(Syn),
                (true, true, false) => Some(SynAck),
                (false, true, false) => Some(Ack),
                (false, false, true) => Some(Fin),
                _ => None,
            };

            // TODO: Check that no data if Syn or SynAck, + tests
            // TODO: Test that decoding fails if none of the above
            // combinations

            if let Some(kind) = maybe_kind {
                let seq_num =
                    u32::from_be_bytes(raw_data[1..5].try_into().unwrap());
                let ack_num =
                    u32::from_be_bytes(raw_data[5..9].try_into().unwrap());

                let data = raw_data[9..].to_vec();

                let seg = Segment {
                    kind,
                    seq_num,
                    ack_num,
                    data,
                };
                Some(seg)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn to_data(self) -> Vec<u8> {
        self.data
    }
}
