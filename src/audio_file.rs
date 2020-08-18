
use hound;
use std::{fs, io};

pub trait AudioFileReader {
    fn next(&mut self) -> Option<u16>;
}

struct WAVFileReader {
    reader: hound::WavIntoSamples<io::BufReader<fs::File>, i32>
}

impl WAVFileReader {
    pub fn new(file: String) -> Self {
        let reader = hound::WavReader::open(file).expect("Failed to open WAV stream");
        let spec = reader.spec();
        if spec.bits_per_sample != 16 || spec.sample_format != hound::SampleFormat::Int || spec.sample_rate != 44100 || spec.channels != 2 {
            panic!("Currently only 44.1kHz Stereo 16 bit WAV files are supported.");
        }

        WAVFileReader {
            reader: reader.into_samples()
        }
    }
}

impl AudioFileReader for WAVFileReader {
    fn next(&mut self) -> Option<u16> {
        self.reader.next().and_then(|s| Some(s.unwrap() as u16))
    }
}

struct FLACReader {
    reader: claxon::FlacReader<fs::File>
}

impl FLACReader {
    pub fn new(file: String) -> Self {
        let mut reader = claxon::FlacReader::open(file.clone()).expect("Failed to open FLAC file");

        let spec = reader.streaminfo();
        if spec.bits_per_sample != 16 || spec.sample_rate != 44100 || spec.channels != 2 {
            panic!("Currently only 44.1kHz Stereo 16 bit FLAC files are supported.");
        }

        let samples: claxon::FlacSamples<&mut claxon::input::BufferedReader<fs::File>> = reader.samples();

        FLACReader {
            reader: reader
        }
    }
}

pub fn open_wave(file: String) -> Box<dyn AudioFileReader> {
    println!("Opening file: {}", file);

    let file_lowercase = file.to_ascii_lowercase();
    if file_lowercase.ends_with(".wav") {
        Box::new(WAVFileReader::new(file))
    } else if file_lowercase.ends_with(".flac") {
        unimplemented!()
        //Box::new(FLACReader::new(file))
    } else {
        panic!("Unsupported file type: {}", file)
    }

}