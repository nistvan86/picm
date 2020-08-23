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

const LEFT_OFFSET: i32 = 14;
const TOP_OFFSET: i32 = 1;

const PCM_LINE_PREAMBLE_BYTES: &'static [u8] = &[1, 0, 1, 0];
const PCM_LINE_PREAMBLE_WIDTH: i32 = PCM_LINE_PREAMBLE_BYTES.len() as i32;
const PCM_LINE_END_WHITE_REFERENCE_BYTES: &'static [u8] = &[0, 2, 2, 2, 2];
const PCM_LINE_END_WHITE_REFERENCE_WIDTH: i32 = PCM_LINE_END_WHITE_REFERENCE_BYTES.len() as i32;

const PCM_FULL_WIDTH: i32 = 137;
const PCM_DATA_WIDTH: i32 = PCM_FULL_WIDTH - PCM_LINE_PREAMBLE_WIDTH - PCM_LINE_END_WHITE_REFERENCE_WIDTH;

const BLACK: RGB8 = RGB8 { r: 0, g: 0, b: 0};
const GRAY: RGB8 = RGB8 { r: 153, g: 153, b: 153 };
const WHITE: RGB8 = RGB8 { r: 255, g: 255, b: 255 };

const DISPMANX_LAYER: i32 = 200;

#[derive(Copy, Clone)]
struct PCMMode {
    screen_width: i32,
    screen_height: i32,
    field_rate: i32,

    visible_pcm_field_height: i32,
    visible_pcm_data_field_height: i32,
    pcm_data_lines_in_field: i32,
}

impl PCMMode {
    fn new(screen_width: i32, screen_height: i32, field_rate: i32, pcm_data_lines_in_field: i32) -> Self {
        let visible_pcm_field_height = screen_height / 2;

        PCMMode {
            screen_width: screen_width,
            screen_height: screen_height,
            field_rate: field_rate,

            visible_pcm_field_height: visible_pcm_field_height,
            visible_pcm_data_field_height: visible_pcm_field_height - 1,
            pcm_data_lines_in_field: pcm_data_lines_in_field
        }
    }
}

#[derive(Clap)]
#[clap(name="PiCM", version = "0.1.2", author = "István Nagy <nistvan.86@gmail.com>")]
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
    let mut line = vec![0u8; PCM_FULL_WIDTH as usize];
    paste(&mut line, 0, Vec::from(PCM_LINE_PREAMBLE_BYTES));
    paste(&mut line, PCM_FULL_WIDTH as usize - PCM_LINE_END_WHITE_REFERENCE_BYTES.len(), Vec::from(PCM_LINE_END_WHITE_REFERENCE_BYTES));
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

    let display = Arc::new(Display::init(0));

    // Try to figure out the PCM mode from current resolution
    let resolution = display.get_resolution();

    let modes: Vec<PCMMode> = Vec::from([
        PCMMode::new(720, 576, 50, 294), // PAL
        PCMMode::new(720, 480, 60, 245) // NTSC
    ]);

    let mut compatible_mode: Option<PCMMode> = None;

    for current_mode in modes {
        if resolution.width == current_mode.screen_width && resolution.height == current_mode.screen_height {
            compatible_mode = Some(current_mode);
            break;
        }
    }

    if compatible_mode.is_none() {
        panic!("Can't found a PCM mode for the current resolution: {}x{}", resolution.width, resolution.height);
    }
    let mode = compatible_mode.unwrap();

    let mut playlist = if opts.input.to_ascii_lowercase().ends_with(".m3u") {
        Playlist::new_from_m3u(opts.input.clone())
    } else {
        Playlist::new_with_single_item(opts.input.clone())
    };

    let ring_buffer: SpscRb<u8> = SpscRb::new((mode.visible_pcm_data_field_height * PCM_DATA_WIDTH * mode.field_rate * 2) as usize);
    let (ring_buffer_producer, ring_buffer_consumer) = (ring_buffer.producer(), ring_buffer.consumer());

    thread::spawn(move || {
        let mut wav_samples = open_wave(playlist.next_file());
        let mut pcm = PCMEngine::new();

        let mut current_line = 0;
        let mut line_pixel_bytes = [0u8; PCM_DATA_WIDTH as usize];

        loop {
            let stereo_sample = next_stereo_samples(&mut wav_samples);
            let samples = if stereo_sample.is_none() {
                wav_samples = open_wave(playlist.next_file()); // Move to next playlist item
                next_stereo_samples(&mut wav_samples).unwrap()
            } else {
                stereo_sample.unwrap()
            };

            if let Some(line_data) = pcm.submit_stereo_sample(samples) {
                if current_line < mode.visible_pcm_data_field_height {
                    bits_to_pixels(line_data, &mut line_pixel_bytes);
                    ring_buffer_producer.write_blocking(&line_pixel_bytes);
                }
                if current_line == mode.pcm_data_lines_in_field - 1 {
                    current_line = 0;
                } else {
                    current_line += 1;
                }
            }
        }
    });

    display.set_bilinear_filtering(false);

    let display_clone = display.clone();

    let draw_thread_handle = thread::spawn(move || {
        set_current_thread_priority(ThreadPriority::Max).expect("Failed to set thread priority");

        let update = display.start_update(10);

        let mut palette = Palette::from_colors(vec![BLACK, GRAY, WHITE]);

        // Synchronization frame
        let mut sync_frame_resource = ImageResource::from_image(Image::new(ImageType::_8BPP, PCM_FULL_WIDTH, 1));
        sync_frame_resource.set_palette(&mut palette);
        let base_line = get_base_line();
        sync_frame_resource.image.set_pixel_bytes(0, 0, &base_line);
        sync_frame_resource.update();

        let frame_rect = Rect { x: LEFT_OFFSET, y: TOP_OFFSET, width: mode.screen_width - LEFT_OFFSET, height: (mode.visible_pcm_field_height * 2) };
        update.create_element_from_image_resource(DISPMANX_LAYER, frame_rect, &sync_frame_resource);

        // Data front and back buffer image resource
        let mut data_resources: Vec<ImageResource> = Vec::new();
        for _ in 0..2 {
            let mut resource = ImageResource::from_image(Image::new(ImageType::_8BPP, PCM_DATA_WIDTH, mode.visible_pcm_field_height));
            resource.set_palette(&mut palette);
            resource.update();
            data_resources.push(resource);
        }

        let physical_pixel_width = (mode.screen_width - LEFT_OFFSET) as f32 / PCM_FULL_WIDTH as f32;
        let data_physical_left_offset = (LEFT_OFFSET as f32 + physical_pixel_width * PCM_LINE_PREAMBLE_WIDTH as f32).round() as i32;
        let data_physical_width = (physical_pixel_width * PCM_DATA_WIDTH as f32).round() as i32;

        let data_rect = Rect { x: data_physical_left_offset, y: TOP_OFFSET, width: data_physical_width, height: (mode.visible_pcm_field_height * 2) };
        let data_element = update.create_element_from_image_resource(DISPMANX_LAYER + 1, data_rect, &data_resources[0]);
        update.submit_sync();

        let mut field_timer = if opts.render_times { Some(AvgPerformanceTimer::new(50)) } else { None };

        let mut next_resource = 0;
        let mut next_field_data = [0u8; PCM_DATA_WIDTH as usize];

        // CTL, last 4 bits: no copyright, P-correction, no Q-correction (16bit mode), no pre-emph
        let ctl_line = pcm::add_crc_to_data(0xCCCCCCCCCCCCCC000000000000000000u128 | (0b0011 << 16));
        let mut ctl_pixel_bytes = [0u8; PCM_DATA_WIDTH as usize];
        bits_to_pixels(ctl_line, &mut ctl_pixel_bytes);

        loop {
            thread::park(); // VSync handler wakes us up
            if let Some(timer) = &mut field_timer { timer.begin(); }

            let update = display.start_update(10);

            next_resource = if next_resource == 1 { 0 } else { 1 };

            for h in 0..mode.visible_pcm_field_height {
                if h == 0 {
                    data_resources[next_resource].image.set_pixel_bytes(0, h, &Vec::from(ctl_pixel_bytes));
                } else {
                    ring_buffer_consumer.read_blocking(&mut next_field_data);
                    data_resources[next_resource].image.set_pixel_bytes(0, h, &Vec::from(next_field_data));
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
