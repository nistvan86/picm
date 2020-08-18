const CRC16_CCITT_POLY: u16 = 0x1021;

fn get_crc16_ccitt_false(data: u128, bits: u8) -> u16 {
    let mut crc = 0xffffu16;

    if bits > 128 { panic!("Bits can be maximum 128 bits") }
    if bits % 8 > 0 { panic!("Bits needs to be divisable by 8") }

    let mut cursor = bits;
    loop {
        let mask = 0xff << cursor - 8;
        let mut byte = ((data & mask) >> cursor - 8) as u16;

        byte = byte << 8;

        for _ in 0..8 {
            let xor_flag = ((crc ^ byte) & 0x8000) > 0;

            crc = crc << 1;

            if xor_flag { 
                crc = crc ^ CRC16_CCITT_POLY;
            }

            byte = byte << 1;
        }
        
        if cursor == 8 { break; }
        cursor-=8;
    }

    crc
}

pub fn add_crc_to_data(data: u128) -> u128 {
    data | get_crc16_ccitt_false(data >> 16, 112) as u128
}

struct FIFODelayer {
    buf: Vec<u16>,
}

impl FIFODelayer {
    fn new(delay_length: u8) -> Self {
        FIFODelayer {
            buf: vec![0u16; (delay_length + 1) as usize]
        }
    }

    fn feed(&mut self, sample: u16) {
        if self.buf.len() > 1 { self.buf.rotate_right(1); }
        self.buf[0] = sample;
    }

    fn get_output(&self) -> u16 {
        self.buf[self.buf.len()-1]
    }
}

pub struct PCMEngine {
    lines: Vec<FIFODelayer>,
    current_line_input: usize,
    last_three_stereo_samples: Vec<[u16; 2]>
}

impl PCMEngine {
    pub fn new() -> Self {
        let mut lines: Vec<FIFODelayer> = Vec::with_capacity(7);
        for d in 0..7 {
            lines.push(FIFODelayer::new(d * 16));
        }

        PCMEngine {
            lines: lines,
            current_line_input: 0,
            last_three_stereo_samples: Vec::with_capacity(3)
        }
    }

    fn get_current_line_data(&self) -> u128 {
        let mut data = 0u128;
        for d in 0..7 { // 14 bit words
            let value = ((self.lines[d].get_output() >> 2) as u128) << (128 - 14*(d+1));
            data = data | value
        }

        let mut s_word = 0u16;
        for d in 0..7 { // 2 bit words (multiplexed into a single S word)
            let data = (self.lines[d].get_output() & 0x3) << (14 - 2*(d+1));
            s_word = s_word | data;
        }
        data = data | ((s_word as u128) << 128 - 14*8);

        add_crc_to_data(data)
    }

    fn get_p_value(&self) -> u16 {
        if self.last_three_stereo_samples.len() < 3 { panic!("Not enough stereo samples."); }
        let mut p = 0u16;
        for stereo_sample in &self.last_three_stereo_samples {
            p = p ^ stereo_sample[0] ^ stereo_sample[1];
        }
        p
    }

    pub fn submit_stereo_sample(&mut self, stereo_sample: [u16; 2]) -> Option<u128> {
        for sample in &stereo_sample { 
            self.lines[self.current_line_input].feed(*sample);
            self.current_line_input += 1; 
        }
        self.last_three_stereo_samples.push(stereo_sample);

        if self.last_three_stereo_samples.len() == 3 {
            // We need to calculate an additional P CRC checksum
            let p_value = self.get_p_value();
            self.lines[self.current_line_input].feed(p_value); // P

            self.last_three_stereo_samples.clear();
            self.current_line_input = 0;

            Some(self.get_current_line_data())
        } else {
            None
        }
    }

}