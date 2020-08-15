mod display;
use display::{Display, Image, Rect, ImageType, ImageResource, Palette, RGB8};

use std::{thread, time};

const WIDTH: i32 = 141;
const HEIGHT: i32 = 576 / 2;

fn main() {

    let display = Display::init(0);

    let mut image = Image::new(ImageType::_8BPP, WIDTH, HEIGHT);
    let mut image_resource = ImageResource::for_image(&mut image);

    let mut palette = Palette::from_colors(vec![RGB8 { r: 0, g: 0, b: 0 }, RGB8 { r: 153, g: 153, b: 153 }, RGB8 { r: 255, g: 255, b: 255 }]);
    image_resource.set_palette(&mut palette);

    image_resource.image.set_pixels_indexed(0, 5, vec![1, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 0, 2, 2, 2, 2]);

    image_resource.update();
    
    let dest_rect = Rect { x: 0, y: 0, width: 720, height: 576 };

    let update = display.start_update(10);

    update.add_element_from_image(200, dest_rect, image_resource);

    update.submit_sync();

    loop {
        thread::sleep(time::Duration::from_secs(1));
    }

}
