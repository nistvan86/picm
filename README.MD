# PiCM

A realtime PCM encoder for the Raspberry Pi.

This software allows you to output digital audio encoded as composite video which is compatible with the SONY PCM-F1 hadware PCM decoder.

## Limitations (at the moment)
- Only 44.1kHz Stereo 16 bit WAV files are supported as input.
- Only PAL video is rendered. Every parameter is hardwired for this at the moment.
- No playback controls, WAV files is being looped as long as you don't terminate the executable.
- Dispmanx resources are not freed explicitly after termination (though it's working this way, it would be nicer to do so)
- Encoding of the bitformat is done in series with the rendering (these two should be separated in the future); background activity can knock the app out of sync and some distrotion might be heard.
- No tests :( Need to figure out how to run them in a cross-compiled environment. Yeah, I'm still learning Rust as I wrote this :)

## Requirements
- Tested with a Raspberry Pi Zero (non W model). Binary is compiled for armv6, so it will not work on other Pi models, thought it might be as easy as modifying the compiler in the `.cargo/config` file and installing the necessary toolchain.
- `sdtv_mode=2` is required in `config.txt`
- Tested with Raspberry OS 10 without graphical frontend (console framebuffer mode). Probably will not work with any desktop running, because of the low level nature of accessing the GPU.

## Usage:

    picm <wav_file_path>

## Inspirations:
- raspi-teletext

## Thanks

* Fagear and mihail2501 on FagearTechCorner Discord channel for providing extreme useful help/tools and had many patience for my silly problems with the format :)