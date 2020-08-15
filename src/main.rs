mod display;
mod timer;

use display::{Display, Image, Rect, ImageType, ImageResource, Palette, RGB8, VSyncData};
use timer::AvgPerformanceTimer;

use std::{thread, time};
use std::sync::Arc;

const WIDTH: i32 = 141;
const HEIGHT: i32 = 576 / 2;

fn main() {

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

    let draw_thread_handle = thread::spawn(move || {
        let mut timer = AvgPerformanceTimer::new(50);
        let mut next_resource = 0;

        let mut data = vec![0u8; WIDTH as usize];
        data.splice(0..4, vec![1, 0, 1, 0]); // Line start
        data.splice(data.len()-5..data.len(), vec![2, 2, 2, 2]); // AGC white line ending

        loop {
            let display = display.start_update(10);

            display.replace_element_source(&element, &resources[next_resource]);

            thread::park(); // VSync handler wakes us up
            thread::sleep(time::Duration::from_micros(2000)); // To miss the current draw and not update in middle of field
            display.submit();

            timer.begin();

            next_resource = if next_resource == 1 { 0 } else { 1 };

            for h in 0..HEIGHT {
                resources[next_resource].image.set_pixels_indexed(0, h, data.clone());
            }
            resources[next_resource].update();

            timer.end();
        }
    });

    let mut vsync_data = VSyncData { draw_thread: &draw_thread_handle.thread() };
    display_clone.start_vsync_handler(&mut vsync_data);
    draw_thread_handle.join().unwrap();

}
