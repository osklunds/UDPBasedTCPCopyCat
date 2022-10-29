#[cfg(test)]
mod tests;

pub struct Segment {
    syn: bool,
    ack: bool,
    fin: bool,

    seq_num: u32,
    ack_num: u32,

    data: Vec<u8>,
}

impl Segment {
    pub fn new(
        syn: bool,
        ack: bool,
        fin: bool,
        seq_num: u32,
        ack_num: u32,
        data: &[u8],
    ) -> Segment {
        Segment {
            syn,
            ack,
            fin,

            seq_num,
            ack_num,

            data: data.to_vec(),
        }
    }

    pub fn syn(&self) -> bool {
        self.syn
    }

    pub fn ack(&self) -> bool {
        self.ack
    }

    pub fn fin(&self) -> bool {
        self.fin
    }

    pub fn seq_num(&self) -> u32 {
        self.seq_num
    }

    pub fn ack_num(&self) -> u32 {
        self.ack_num
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::new();

        let mut first_byte: u8 = 0;
        first_byte |= (self.syn as u8) << 0;
        first_byte |= (self.ack as u8) << 1;
        first_byte |= (self.fin as u8) << 2;
        encoded.push(first_byte);

        encoded.extend_from_slice(&self.seq_num.to_be_bytes());
        encoded.extend_from_slice(&self.ack_num.to_be_bytes());
        encoded.extend_from_slice(&self.data);

        encoded
    }
}
