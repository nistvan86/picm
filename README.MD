# PiCM

A realtime PCM encoder for the Raspberry Pi.

This software allows you to output digital audio encoded as composite video which is compatible with the SONY PCM-F1's video format (not tested with actual hardware yet, probably needs the `tvctl` command from the `raspi-teletext` project to shift the top of the picture to the VBI area).
You can either supply a single wav file or an m3u playlist file with wav files to be played.

Watch it in action: https://www.youtube.com/watch?v=WRrcjgK-Pc8&feature=youtu.be

## Limitations (at the moment)
- Only 44.1kHz Stereo 16 bit WAV files are supported as input.
- Only PAL video is rendered. Every parameter is hardwired for this at the moment.
- No playback controls, WAV files are being looped as long as you don't terminate the executable.
- Dispmanx resources are not freed explicitly after termination (though it's working this way, it would be nicer to do so)
- No tests :( Need to figure out how to run them in a cross-compiled environment. Yeah, I'm still learning Rust as I wrote this :)

## Requirements
- Tested with a Raspberry Pi Zero (non W model, but should work with that as well). Binary is compiled for armv6, so it will not work on other Pi models; it should be as easy as modifying the compiler in the `.cargo/config` file and installing the necessary toolchain.
- `sdtv_mode=2` is required in `config.txt`
- Tested with Raspberry OS 10 without graphical frontend (console framebuffer mode). Probably will not work with any desktop running, because of the low level nature of accessing the GPU.

## Usage:

    picm [wav_file_path | m3u_file_path]

## Inspirations:
- raspi-teletext: https://github.com/ali1234/raspi-teletext

## Thanks

* Fagear and mihail2501 on FagearTechCorner Discord channel for providing extreme useful help/tools and had many patience for my silly problems with the format :)
* The author of this project, for the Dispmanx setup examples: https://github.com/AndrewFromMelbourne/raspidmx
