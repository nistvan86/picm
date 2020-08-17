//const MIN_NUM_STEREO_SAMPLES: u16 = 336;
const CRC16_CCIT_POLY: u16 = 0x1021;

fn get_crc16(data: u128, bits: u8) -> u16 {
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
                crc = crc ^ CRC16_CCIT_POLY;
            }

            byte = byte << 1;
        }
        
        if cursor == 8 { break; }
        cursor-=8;
    }

    crc
}

pub fn add_crc_to_data(data: u128) -> u128 {
    data | get_crc16(data >> 16, 112) as u128
}

struct Delayer {
    buf: Vec<u16>,
}

impl Delayer {
    fn new(delay_length: u8) -> Self {
        Self {
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
    lines: Vec<Delayer>,
    current_line_input: usize,
    last_three_stereo_sample: Vec<[u16; 2]>,
    //init_sample_counter: u16
}

impl PCMEngine {
    pub fn new() -> Self {
        let mut delayers: Vec<Delayer> = Vec::with_capacity(8);
        for d in 0..8 {
            delayers.push(Delayer::new(d * 16));
        }

        PCMEngine {
            lines: delayers,
            current_line_input: 0,
            last_three_stereo_sample: Vec::with_capacity(3),
            //init_sample_counter: 0
        }
    }

    fn submit_sample_to_delayer(&mut self, sample: u16) {
        self.lines[self.current_line_input].feed(sample);
        self.current_line_input = if self.current_line_input == 7 { 0 } else { self.current_line_input + 1 }
    }

    fn get_current_line_data(&self) -> u128 {
        let mut data = 0u128;
        for d in 0..8 {
            let output_shifted = ((self.lines[d].get_output() >> 2) as u128) << (128 - 14*(d+1));
            data = data | output_shifted
        }

        add_crc_to_data(data)
    }

    fn get_p_value(&self) -> u16 {
        if self.last_three_stereo_sample.len() < 3 { panic!("Not enough stereo samples."); }
        let mut p = 0u16;
        for stereo_sample in &self.last_three_stereo_sample {
            p = p ^ (stereo_sample[0]) >> 2 ^ (stereo_sample[1] >> 2);
        }
        p
    }

    fn get_s_value(&self) -> u16 {
        if self.last_three_stereo_sample.len() < 3 { panic!("Not enough stereo samples."); }

        let mut p = 0u16;
        let mut pos = 12;
        let mut xor = 0u8;
        for stereo_sample in &self.last_three_stereo_sample {
            let left_bits = stereo_sample[0] & 3;
            let right_bits = stereo_sample[1] & 3;
            p = p | (left_bits << pos);
            pos-=2;
            p = p | (right_bits << pos);
            pos-=2;
            xor = xor ^ left_bits as u8 ^ right_bits as u8;
        }
        p = p | xor as u16;
        p
    }

    pub fn submit_stereo_sample(&mut self, stereo_sample: [u16; 2]) -> Option<u128> {
        for sample in &stereo_sample { self.submit_sample_to_delayer(*sample); }
        self.last_three_stereo_sample.push(stereo_sample);

        //if self.init_sample_counter < MIN_NUM_STEREO_SAMPLES { self.init_sample_counter+=1 }

        if self.last_three_stereo_sample.len() == 3 {
            // We need to calculate an additional P CRC checksum and Q for the extra 2 bits / sample

            self.submit_sample_to_delayer(self.get_p_value() << 2); // P
            self.submit_sample_to_delayer(self.get_s_value() << 2); // S (16 bit extension)

            self.last_three_stereo_sample.clear();

            //if self.init_sample_counter == MIN_NUM_STEREO_SAMPLES {
                Some(self.get_current_line_data())
            /*} else {
                None
            }*/
        } else {
            None
        }
    }

}