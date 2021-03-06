# PiCM

A realtime PCM encoder for the Raspberry Pi.

This software allows you to output digital audio encoded as composite video which is compatible with the SONY PCM-F1's video format.

Note: not tested with actual hardware yet, probably needs the `tvctl` command from the `raspi-teletext` project to shift the top of the picture to the VBI area to be detected.

You can either supply a single wav file or an m3u playlist file with wav files to be played.

Watch it in action: https://www.youtube.com/watch?v=WRrcjgK-Pc8&feature=youtu.be

## Limitations (at the moment)
- Only 44.1kHz Stereo 16 bit WAV files are supported as input. (no resampling, so in NTSC mode audio is played ~0,001% slower)
- Only renders 576/480 lines; the format can still keep up and correct the missing lines, but only in ideal conditions (haven't tried dubbing it to VHS and playing it back from there). Would be nice to figure out how to increase the Raspberry's composite output height to let it render more lines from framebuffer, but it's unlikely.
- No playback controls, files are being looped as long as you don't terminate the executable.
- m3u support is very minimalistic (can't handle absolute paths or empty lines, oops). In a folder of files use `ls *.wav >playlist.m3u` for best result.
- Dispmanx resources are not freed explicitly after termination (though it's working this way, it would be nicer to do so)
- No tests (need to figure out how to run them in a cross-compiled environment)
- Haven't tested compilation on actual device, probably needs tuning of toolchain in `./cargo/config`.

## Requirements
- Tested with a Raspberry Pi Zero (non W model, but should work with that as well). Binary is compiled for armv6, so it will not work on other Pi models; though it should be as easy as modifying the compiler in the `.cargo/config` file and installing the necessary toolchain.
- For PAL mode `sdtv_mode=2`, for NTSC mode `sdtv_mode=0` is required in `config.txt`
- Tested with Raspberry OS 10 without graphical frontend (console framebuffer mode). Probably will not work with any desktop running, because of the low level nature of accessing the GPU.
- It's recommended to overclock the Pi in `config.txt` to reduce sync issues. Also minimize any background activity if you want the best result. (even disconnect from network, play WAV files from SD card instead of USB stick, etc.) Though unfortunately the Vsync callback handler of the Pi is not perfect and sometimes cause tearing which corrupts the data, despite all my efforts so far.

## Usage:

    picm [wav_file_path | m3u_file_path]

## Inspirations:
- raspi-teletext: https://github.com/ali1234/raspi-teletext

## Thanks

* Fagear and mihail2501 on FagearTechCorner Discord channel for providing extreme useful help/tools and had many patience for my silly problems with the format :)
* The author of this project, for the Dispmanx setup examples: https://github.com/AndrewFromMelbourne/raspidmx
