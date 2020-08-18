mod display;
mod timer;
mod pcm;
mod playlist;

use display::{Display, Image, Rect, ImageType, ImageResource, Palette, RGB8, VSyncData};
use pcm::PCMEngine;
use timer::AvgPerformanceTimer;
use playlist::Playlist;

use std::{thread, io, fs, sync::Arc};
use hound;
use clap::Clap;
use thread_priority::*;
use rb::*;

const WIDTH: i32 = 138;
const DATA_WIDTH: i32 = WIDTH - LINE_PREAMBLE.len() as i32 - LINE_PREAMBLE.len() as i32;
const HEIGHT: i32 = 576 / 2;

const LEFT_OFFSET: i32 = 14;
const TOP_OFFSET: i32 = 1;

const TOTAL_LINES_IN_FIELD: i32 = 295; // PAL 590 line PCM resolution, 295 lines in field (incl. CTL line)
const TOTAL_DATA_LINES_IN_FIELD: i32 = TOTAL_LINES_IN_FIELD - 1;
const RINGBUFFER_SIZE: i32 = TOTAL_DATA_LINES_IN_FIELD * 8; // 8 fields in advance

const LINE_PREAMBLE: &'static [u8] = &[1, 0, 1, 0];
const LINE_END_WHITE_REFERENCE: &'static [u8] = &[0, 2, 2, 2, 2];

const BLACK: RGB8 = RGB8 { r: 0, g: 0, b: 0};
const GRAY: RGB8 = RGB8 { r: 153, g: 153, b: 153 };
const WHITE: RGB8 = RGB8 { r: 255, g: 255, b: 255 };

const DISPMANX_LAYER: i32 = 200;

const CONTROL_LINE_BASE: u128 = 0xCCCCCCCCCCCCCC000000000000000000u128;

#[derive(Clap)]
#[clap(name="PiCM", version = "0.1.0", author = "István Nagy <nistvan.86@gmail.com>")]
struct Opts {
    /// .wav or .m3u file pointing to WAV files to be played.
    input: String,
}

fn paste(v: &mut Vec<u8>, x: usize, p: Vec<u8>) {
    v.splice(x..x + p.len(), p);
}

fn get_base_line() -> Vec<u8> {
    let mut line = vec![0u8; WIDTH as usize];
    paste(&mut line, 0, Vec::from(LINE_PREAMBLE));
    paste(&mut line, WIDTH as usize - LINE_END_WHITE_REFERENCE.len() - 1, Vec::from(LINE_END_WHITE_REFERENCE));
    line
}

/*fn bits_to_line_pixels(bits: u128) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::with_capacity(128);

    let mut cursor = 1u128 << 127;
    loop {
        data.push(if bits & cursor == cursor { 1 } else { 0 });
        if cursor == 1 { 
            break; 
        } else {
            cursor = cursor >> 1;
        }
    }

    data
}*/


fn next_stereo_samples(wav_samples: &mut hound::WavIntoSamples<io::BufReader<fs::File>, i32>) -> Option<[u16; 2]> {
    let mut result = [0u16; 2];

    for i in 0..2 {
        match wav_samples.next() {
            Some(sample) => {
                result[i] = sample.ok().unwrap() as u16;
            },
            None => return None
        }
    }

    Some(result)
}

fn open_wave(file: String) -> hound::WavIntoSamples<io::BufReader<fs::File>, i32> {
    let reader = hound::WavReader::open(file).unwrap();
    let spec = reader.spec();
    if spec.bits_per_sample != 16 || spec.sample_format != hound::SampleFormat::Int || spec.sample_rate != 44100 || spec.channels != 2 {
        panic!("Currently only 44.1kHz Stereo 16 bit WAV files are supported.");
    }
    reader.into_samples::<i32>()
}

fn main() {
    let opts: Opts = Opts::parse();

    let mut playlist = if opts.input.to_ascii_lowercase().ends_with(".m3u") {
        Playlist::new_from_m3u(opts.input.clone())
    } else {
        Playlist::new_with_single_item(opts.input.clone())
    };

    let ring_buffer: SpscRb<u128> = SpscRb::new(RINGBUFFER_SIZE as usize);
    let (ring_buffer_producer, ring_buffer_consumer) = (ring_buffer.producer(), ring_buffer.consumer());

    thread::spawn(move || {
        let mut wav_samples = open_wave(playlist.next_file());
        let mut pcm = PCMEngine::new();

        let mut result: Vec<u128> = vec![];

        loop {
            let stereo_sample = next_stereo_samples(&mut wav_samples);
            let samples = if stereo_sample.is_none() {
                wav_samples = open_wave(playlist.next_file()); // Move to next playlist item
                next_stereo_samples(&mut wav_samples).unwrap()
            } else {
                stereo_sample.unwrap()
            };

            let line_data = pcm.submit_stereo_sample(samples);

            if line_data.is_some() {
                result.push(line_data.unwrap());
            }

            if result.len() == TOTAL_DATA_LINES_IN_FIELD as usize {
                ring_buffer_producer.write_blocking(&result);
                result.clear();
            }
        }
    });

    let display = Arc::new(Display::init(0));
    display.set_bilinear_filtering(false);

    let display_clone = display.clone();

    let draw_thread_handle = thread::spawn(move || {
        set_current_thread_priority(ThreadPriority::Max).expect("Failed to set thread priority");

        let update = display.start_update(10);

        // Synchronization frame
        let mut palette_4bpp = Palette::from_colors(vec![BLACK, GRAY, WHITE]);
        let mut sync_frame_resource = ImageResource::from_image(Image::new(ImageType::_8BPP, WIDTH, 1));
        sync_frame_resource.set_palette(&mut palette_4bpp);
        sync_frame_resource.image.set_pixel_bytes(0, 0, get_base_line());
        sync_frame_resource.update();

        let frame_rect = Rect { x: LEFT_OFFSET, y: TOP_OFFSET, width: 720 - LEFT_OFFSET, height: (HEIGHT * 2) };
        update.create_element_from_image_resource(DISPMANX_LAYER, frame_rect, &sync_frame_resource);

        // Data front and back buffer image resource
        let mut palette_1bpp = Palette::from_colors(vec![GRAY]);
        let mut data_resources: Vec<ImageResource> = Vec::new();
        for _ in 0..2 {
            let mut resource = ImageResource::from_image(Image::new(ImageType::_1BPP, DATA_WIDTH, HEIGHT));
            resource.set_palette(&mut palette_1bpp);
            resource.update();
            data_resources.push(resource);
        }

        let physical_pixel_width = (720 - LEFT_OFFSET) / WIDTH;
        let data_physical_left_offset = LEFT_OFFSET + (physical_pixel_width * ((LINE_PREAMBLE.len() + 1) as i32));
        let data_physical_width = physical_pixel_width * (DATA_WIDTH + 2);

        let data_rect = Rect { x: data_physical_left_offset, y: TOP_OFFSET, width: data_physical_width, height: (HEIGHT * 2) };
        let data_element = update.create_element_from_image_resource(DISPMANX_LAYER + 1, data_rect, &data_resources[0]);
        update.submit_sync();

        let mut timer = AvgPerformanceTimer::new(50);
        let mut next_resource = 0;

        let mut next_field_data = [0u128; TOTAL_DATA_LINES_IN_FIELD as usize];

        // CTL, last 4 bits: no copyright, P-correction, no Q-correction (16bit mode), no pre-emph
        let ctl_line = pcm::add_crc_to_data(CONTROL_LINE_BASE | (0b00000000000011 << 16));

        loop {
            thread::park(); // VSync handler wakes us up
            timer.begin();

            let update = display.start_update(10);

            next_resource = if next_resource == 1 { 0 } else { 1 };

            ring_buffer_consumer.read_blocking(&mut next_field_data);

            for h in 0..HEIGHT {
                data_resources[next_resource].image.set_pixel_bytes(0, h, if h == 0 { ctl_line } else { next_field_data[(h-1) as usize] }.to_be_bytes().to_vec());
            }

            data_resources[next_resource].update();
            update.replace_element_source(&data_element, &data_resources[next_resource]);
            update.submit();

            timer.end();
        }
    });

    let mut vsync_data = VSyncData { draw_thread: &draw_thread_handle.thread() };
    display_clone.start_vsync_handler(&mut vsync_data);
    draw_thread_handle.join().unwrap();

}
