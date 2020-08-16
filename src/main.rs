mod display;
mod timer;

use display::{Display, Image, Rect, ImageType, ImageResource, Palette, RGB8, VSyncData};
use timer::AvgPerformanceTimer;

use std::{thread, time, io, fs, sync::Arc};

use hound;

const WIDTH: i32 = 137;
const HEIGHT: i32 = 576 / 2;

const CRC16_CCIT_POLY: u16 = 0x1021;

static LINE_PREAMBLE: &'static [u8] = &[1, 0, 1, 0];
static LINE_END_WHITE_REFERENCE: &'static [u8] = &[0, 2, 2, 2, 2];

const CONTROL_LINE_BASE: u128 = 0xCCCCCCCCCCCCCC000000000000000000u128;

fn paste(v: &mut Vec<u8>, x: usize, p: Vec<u8>) {
    v.splice(x..x + p.len(), p);
}

fn get_base_line() -> Vec<u8> {
    let mut line = vec![0u8; WIDTH as usize];
    paste(&mut line, 0, Vec::from(LINE_PREAMBLE));
    paste(&mut line, WIDTH as usize - LINE_END_WHITE_REFERENCE.len() - 1, Vec::from(LINE_END_WHITE_REFERENCE));
    line
}

fn bits_to_line_data(bits: u128) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::with_capacity(128);

    let mut cursor = 1u128 << 127;
    loop {
        data.push(if bits & cursor == cursor { 1 } else { 0 });

        cursor = cursor >> 1;
        if cursor == 1 { break; }
    }

    data
}

fn get_crc16(data: u128, bits: u8) -> u16 {
    let mut crc = 0xffffu16;

    let mut cursor = 1u128 << bits - 1;
    loop {
        let xor_flag = crc & 0x8000 > 0;

        crc = crc << 1;

        if data & cursor > 0 {
            crc += 1;
        }

        if xor_flag { 
            crc = crc ^ CRC16_CCIT_POLY;
        }

        if cursor == 1 { break; }
        cursor = cursor >> 1;
    }

    for _ in 0..16 {
        let xor_flag = crc & 0x8000 > 0;
        crc = crc << 1;

        if xor_flag { crc = crc ^ CRC16_CCIT_POLY }
    }

    crc
}

fn next_samples(wav_samples: &mut hound::WavIntoSamples<io::BufReader<fs::File>, i32>) -> [i32; 6] {
    let mut result = [0i32; 6];

    for i in 0..6 {
        match wav_samples.next() {
            Some(sample) => result[i] = sample.ok().unwrap(),
            None => result[i] = 0
        }
    }

    result
}

fn main() {

    let mut wav_samples = hound::WavReader::open("test.wav").unwrap().into_samples::<i32>();

    let display = Arc::new(Display::init(0));

    let mut palette = Palette::from_colors(vec![RGB8 { r: 0, g: 0, b: 0 }, RGB8 { r: 153, g: 153, b: 153 }, RGB8 { r: 255, g: 255, b: 255 }]);

    let mut resources: Vec<ImageResource> = Vec::new();
    for _ in 0..2 {
        let mut resource = ImageResource::from_image(Image::new(ImageType::_8BPP, WIDTH, HEIGHT));
        resource.set_palette(&mut palette);
        resource.update();
        resources.push(resource);
    }
    
    let dest_rect = Rect { x: 20, y: 0, width: 700, height: 576 };

    let update = display.start_update(10);
    let element = update.create_element_from_image_resource(200, dest_rect, &resources[0]);
    update.submit_sync();

    let display_clone = display.clone();

    let base_line = get_base_line();

    let draw_thread_handle = thread::spawn(move || {
        let mut timer = AvgPerformanceTimer::new(50);
        let mut next_resource = 0;

        loop {
            let display = display.start_update(10);

            display.replace_element_source(&element, &resources[next_resource]);

            thread::park(); // VSync handler wakes us up
            thread::sleep(time::Duration::from_micros(2000)); // To miss the current draw and not update in middle of field
            display.submit();

            timer.begin();

            next_resource = if next_resource == 1 { 0 } else { 1 };

            for h in 0..HEIGHT {
                let mut line = base_line.clone();

                let mut bits = 0u128;

                if h == 0 { // Control line
                    bits = CONTROL_LINE_BASE;
                    //bits = bits | (0b00000000000110 << 16); // CTL: no copyright, P-correction, Q-correction, no pre-emph
                } else {
                    let six_samples = next_samples(&mut wav_samples);
                    for s in 0..6 {
                        let s14 = six_samples[s] as u16 >> 2; // 14 bit part
                        bits = bits | ((s14 as u128) << (128 - ((s+1)*14)));
                    }
                }

                // Add lasts 16 bit CRC
                let upper_112bit = bits >> 16;
                let crc = get_crc16(upper_112bit, 112);
                bits = bits | crc as u128;

                // Convert to line data
                let bits_as_line_data = bits_to_line_data(bits);
                paste(&mut line, 4, bits_as_line_data);

                resources[next_resource].image.set_pixels_indexed(0, h, line);
            }
            resources[next_resource].update();

            timer.end();
        }
    });

    let mut vsync_data = VSyncData { draw_thread: &draw_thread_handle.thread() };
    display_clone.start_vsync_handler(&mut vsync_data);
    draw_thread_handle.join().unwrap();

}
