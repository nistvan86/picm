use videocore::{bcm_host, dispmanx, image::ImageType as VCImageType, image::Rect as VCRect};
use std::ffi::{c_void, CString};
use std::ptr;
use std::thread;
use std::os::raw::c_char;

const NO_ALPHA: dispmanx::VCAlpha = dispmanx::VCAlpha { flags: dispmanx::FlagsAlpha::FIXED_ALL_PIXELS, opacity: 255, mask: 0 };

#[cfg(target_arch = "arm")]
#[link(name = "bcm_host")]
extern {
    fn vc_gencmd_send(format: *const c_char, ...) -> i32;
}

#[derive(Copy, Clone)]
pub enum ImageType {
    _8BPP,
    _1BPP
}

pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32
}

fn rect_to_vc_rect(rect: Rect) -> VCRect {
    VCRect { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
}

fn image_type_to_vc_image_type(image_type: ImageType) -> VCImageType {
    match image_type {
        ImageType::_8BPP => VCImageType::_8BPP,
        ImageType::_1BPP => VCImageType::_1BPP
    }
}

pub struct Display {
    handle: dispmanx::DisplayHandle
}

pub struct VSyncData<'t> {
    pub draw_thread: &'t thread::Thread
}

extern "C" fn vsync_callback(_: dispmanx::UpdateHandle, arg: *mut c_void) {
    let data: &mut VSyncData = unsafe { &mut *(arg as *mut VSyncData) };
    data.draw_thread.unpark();
}

extern "C" fn null_callback(_: dispmanx::UpdateHandle, _: *mut c_void) { }

fn get_c_string(text: &'static str) -> CString {
    CString::new(text).expect("Failed to get CString")
}

impl<'d> Display {
    pub fn init(display: u32) -> Self {
        bcm_host::init();
        let disp_handle = dispmanx::display_open(display);

        Display {
            handle: disp_handle
        }
    }

    pub fn set_bilinear_filtering(&'d self, enabled: bool) {
        unsafe {
            if enabled {
                vc_gencmd_send(get_c_string("%s").as_ptr(), get_c_string("scaling_kernel 0 -2 -6 -8 -10 -8 -3 2 18 50 82 119 155 187 213 227 227 213 187 155 119 82 50 18 2 -3 -8 -10 -8 -6 -2 0 0").as_ptr());
            } else {
                vc_gencmd_send(get_c_string("%s").as_ptr(), get_c_string("scaling_kernel 0 0 0 0 0 0 0 0 1 1 1 1 255 255 255 255 255 255 255 255 1 1 1 1 0 0 0 0 0 0 0 0 1").as_ptr());
            }
        }
    }

    pub fn start_update(&'d self, priority: i32) -> Update<'d> {
        let update_handle = dispmanx::update_start(priority);

        Update {
            display: self,
            handle: update_handle
        }
    }

    pub fn start_vsync_handler(&self, vsync_data: &mut VSyncData) {
        dispmanx::vsync_callback(self.handle, vsync_callback, vsync_data as *mut _ as *mut c_void);
    }
}

pub struct Element {
    handle: dispmanx::ElementHandle
}

pub struct Update<'d> {
    display: &'d Display,
    handle: dispmanx::UpdateHandle
}

impl<'d> Update<'d> {
    pub fn create_element_from_image_resource(&'d self, layer: i32, dest_rect: Rect, image_resource: &ImageResource) -> Element {
        let mut dest_rect_vc = rect_to_vc_rect(dest_rect);
        let mut src_rect_vc = rect_to_vc_rect(image_resource.image.get_src_rect());
        let handle = dispmanx::element_add(self.handle, self.display.handle, layer, &mut dest_rect_vc, image_resource.resource, &mut src_rect_vc, dispmanx::DISPMANX_PROTECTION_NONE, &mut NO_ALPHA, ptr::null_mut(), dispmanx::Transform::NO_ROTATE);
        Element { handle: handle }
    }

    pub fn replace_element_source(&'d self, element: &Element, image_resource: &ImageResource) {
        dispmanx::element_change_source(self.handle, element.handle, image_resource.resource);
    }

    pub fn submit_sync(&self) {
        dispmanx::update_submit_sync(self.handle);
    }

    pub fn submit(&self) {
        dispmanx::update_submit(self.handle, null_callback, ptr::null_mut());
    }
}

#[derive(Copy, Clone)]
pub struct RGB8 {
    pub r: u8,
    pub g: u8,
    pub b: u8
}

pub struct Palette {
    data: Vec<u16>
}

fn rgb_to_16bit(rgb: RGB8) -> u16 {
    ((rgb.r as u16 >> 3) << 11) | ((rgb.g as u16 >> 2) << 5) | (rgb.b as u16 >> 3)
}

impl Palette {
    pub fn from_colors(colors: Vec<RGB8>) -> Self {
        Palette {
            data: colors.into_iter().map(|r| rgb_to_16bit(r)).collect()
        }
    }

    pub fn get_data_ptr(&mut self) -> *mut c_void {
        self.data.as_mut_ptr() as *mut c_void
    }
}

pub struct Image {
    image_type: ImageType,
    pub width: i32,
    pub height: i32,
    pub pitch: i32,
    pub aligned_height: i32,
    data: Vec<u8>
}

fn align_to_16(x: i32) -> i32 {
    (x + 15) & !15
}

impl Image {
    pub fn new(image_type: ImageType, width: i32, height: i32) -> Self {
        let aligned_height: i32 = align_to_16(height);

        match image_type {
            ImageType::_8BPP => {
                let bps: u8 = 8;
                let pitch: i32 = (align_to_16(width) * bps as i32) / 8;
                let data = vec![0u8; (pitch * aligned_height) as usize];

                Self {
                    image_type: image_type,
                    width: width,
                    height: height,
                    pitch: pitch,
                    aligned_height: aligned_height,
                    data: data
                }
            },
            ImageType::_1BPP => {
                let bps: u8 = 1;
                let pitch: i32 = (align_to_16(width) * bps as i32) / 8;
                let data = vec![0u8; (pitch * aligned_height) as usize];

                Self {
                    image_type: image_type,
                    width: width,
                    height: height,
                    pitch: pitch,
                    aligned_height: aligned_height,
                    data: data
                }
            }
        }
    }

    pub fn set_pixel_bytes(&mut self, x: i32, y: i32, bytes: Vec<u8>) {
        let offset = (x + y * self.pitch) as usize;
        let end = offset + bytes.len() as usize;
        self.data.splice(offset..end, bytes.into_iter());
    }

    pub fn get_data_ptr(&mut self) -> *mut c_void {
        self.data.as_mut_ptr() as *mut c_void
    }

    pub fn get_src_rect(&self) -> Rect {
        Rect { x: 0, y: 0, width: self.width << 16, height: self.height << 16 }
    }
}

pub struct ImageResource {
    pub image: Image,
    pub resource: dispmanx::ResourceHandle
}

impl<'a> ImageResource {
    pub fn from_image(image: Image) -> Self {
        let mut _ptr: u32 = 0;
        let resource = dispmanx::resource_create(image_type_to_vc_image_type(image.image_type), (image.width | (image.pitch << 16)) as u32, (image.height | (image.aligned_height << 16)) as u32, &mut _ptr);

        Self {
            image: image,
            resource: resource
        }
    }

    pub fn set_palette(&self, palette: &mut Palette) {
        dispmanx::resource_set_palette(self.resource, palette.get_data_ptr(), 0, palette.data.len() as i32 * 2);
    }

    pub fn update(&mut self) {
        let rect = VCRect { x: 0, y: 0, width: self.image.width, height: self.image.height };
        dispmanx::resource_write_data(self.resource, image_type_to_vc_image_type(self.image.image_type), self.image.pitch, self.image.get_data_ptr(), &rect);
    }

}