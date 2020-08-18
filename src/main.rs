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

const SCREEN_WIDTH: i32 = 720;
const SCREEN_HEIGHT: i32 = 576;
const LEFT_OFFSET: i32 = 14;
const TOP_OFFSET: i32 = 1;

const FULL_WIDTH: i32 = 137;

const DATA_WIDTH: i32 = FULL_WIDTH - LINE_PREAMBLE_WIDTH - LINE_END_WHITE_REFERENCE_WIDTH;
const DATA_FIELD_HEIGHT: i32 = SCREEN_HEIGHT / 2;

const TOTAL_LINES_IN_FIELD: i32 = 295; // PAL 590 line PCM resolution, 295 lines in field (incl. CTL line)
const TOTAL_DATA_LINES_IN_FIELD: i32 = TOTAL_LINES_IN_FIELD - 1;
const RINGBUFFER_SIZE: i32 = TOTAL_DATA_LINES_IN_FIELD * 8; // 8 fields in advance

const LINE_PREAMBLE_BYTES: &'static [u8] = &[1, 0, 1, 0];
const LINE_PREAMBLE_WIDTH: i32 = LINE_PREAMBLE_BYTES.len() as i32;
const LINE_END_WHITE_REFERENCE_BYTES: &'static [u8] = &[0, 2, 2, 2, 2];
const LINE_END_WHITE_REFERENCE_WIDTH: i32 = LINE_END_WHITE_REFERENCE_BYTES.len() as i32;

const BLACK: RGB8 = RGB8 { r: 0, g: 0, b: 0};
const GRAY: RGB8 = RGB8 { r: 153, g: 153, b: 153 };
const WHITE: RGB8 = RGB8 { r: 255, g: 255, b: 255 };

const DISPMANX_LAYER: i32 = 200;

#[derive(Clap)]
#[clap(name="PiCM", version = "0.1.1", author = "István Nagy <nistvan.86@gmail.com>")]
struct Opts {
    /// .wav or .m3u file pointing to WAV files to be played.
    input: String,
    /// Print field render average times every second
    #[clap(short)]
    render_times: bool,
}

fn paste(v: &mut Vec<u8>, x: usize, p: Vec<u8>) {
    v.splice(x..x + p.len(), p);
}

fn get_base_line() -> Vec<u8> {
    let mut line = vec![0u8; FULL_WIDTH as usize];
    paste(&mut line, 0, Vec::from(LINE_PREAMBLE_BYTES));
    paste(&mut line, FULL_WIDTH as usize - LINE_END_WHITE_REFERENCE_BYTES.len(), Vec::from(LINE_END_WHITE_REFERENCE_BYTES));
    line
}

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
    println!("Opening WAV: {}", file);
    let reader = hound::WavReader::open(file).unwrap();
    let spec = reader.spec();
    if spec.bits_per_sample != 16 || spec.sample_format != hound::SampleFormat::Int || spec.sample_rate != 44100 || spec.channels != 2 {
        panic!("Currently only 44.1kHz Stereo 16 bit WAV files are supported.");
    }
    reader.into_samples::<i32>()
}

fn bits_to_pixels(bits: u128, pixel_bytes: &mut [u8; 128]) {
    const SOURCE_BITS: u8 = 128;
    for b in 0..SOURCE_BITS {
        pixel_bytes[b as usize] = if (bits & (1 << SOURCE_BITS-1-b)) > 0 { 1 } else { 0 };
    }
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

        let mut palette = Palette::from_colors(vec![BLACK, GRAY, WHITE]);

        // Synchronization frame
        let mut sync_frame_resource = ImageResource::from_image(Image::new(ImageType::_8BPP, FULL_WIDTH, 1));
        sync_frame_resource.set_palette(&mut palette);
        let base_line = get_base_line();
        sync_frame_resource.image.set_pixel_bytes(0, 0, &base_line);
        sync_frame_resource.update();

        let frame_rect = Rect { x: LEFT_OFFSET, y: TOP_OFFSET, width: SCREEN_WIDTH - LEFT_OFFSET, height: (DATA_FIELD_HEIGHT * 2) };
        update.create_element_from_image_resource(DISPMANX_LAYER, frame_rect, &sync_frame_resource);

        // Data front and back buffer image resource
        let mut data_resources: Vec<ImageResource> = Vec::new();
        for _ in 0..2 {
            let mut resource = ImageResource::from_image(Image::new(ImageType::_8BPP, DATA_WIDTH, DATA_FIELD_HEIGHT));
            resource.set_palette(&mut palette);
            resource.update();
            data_resources.push(resource);
        }

        let physical_pixel_width = (SCREEN_WIDTH - LEFT_OFFSET) as f32 / FULL_WIDTH as f32;
        let data_physical_left_offset = (LEFT_OFFSET as f32 + physical_pixel_width * LINE_PREAMBLE_WIDTH as f32).round() as i32;
        let data_physical_width = (physical_pixel_width * DATA_WIDTH as f32).round() as i32;

        let data_rect = Rect { x: data_physical_left_offset, y: TOP_OFFSET, width: data_physical_width, height: (DATA_FIELD_HEIGHT * 2) };
        let data_element = update.create_element_from_image_resource(DISPMANX_LAYER + 1, data_rect, &data_resources[0]);
        update.submit_sync();

        let mut field_timer = if opts.render_times { Some(AvgPerformanceTimer::new(50)) } else { None };

        let mut next_resource = 0;
        let mut next_field_data = [0u128; TOTAL_DATA_LINES_IN_FIELD as usize];

        // CTL, last 4 bits: no copyright, P-correction, no Q-correction (16bit mode), no pre-emph
        let ctl_line = pcm::add_crc_to_data(0xCCCCCCCCCCCCCC000000000000000000u128 | (0b0011 << 16));
        let mut ctl_pixel_bytes = [0u8; DATA_WIDTH as usize];
        bits_to_pixels(ctl_line, &mut ctl_pixel_bytes);

        let mut data_pixel_bytes = [0u8; DATA_WIDTH as usize];

        loop {
            thread::park(); // VSync handler wakes us up
            if let Some(timer) = &mut field_timer { timer.begin(); }

            let update = display.start_update(10);

            next_resource = if next_resource == 1 { 0 } else { 1 };

            ring_buffer_consumer.read_blocking(&mut next_field_data);

            for h in 0..DATA_FIELD_HEIGHT {
                if h == 0 {
                    data_resources[next_resource].image.set_pixel_bytes(0, h, &Vec::from(ctl_pixel_bytes));
                } else {
                    bits_to_pixels(next_field_data[(h-1) as usize], &mut data_pixel_bytes);
                    data_resources[next_resource].image.set_pixel_bytes(0, h, &Vec::from(data_pixel_bytes));
                }
            }

            data_resources[next_resource].update();
            update.replace_element_source(&data_element, &data_resources[next_resource]);
            update.submit();

            if let Some(timer) = &mut field_timer { timer.end(); }
        }
    });

    let mut vsync_data = VSyncData { draw_thread: &draw_thread_handle.thread() };
    display_clone.start_vsync_handler(&mut vsync_data);
    draw_thread_handle.join().unwrap();
}
