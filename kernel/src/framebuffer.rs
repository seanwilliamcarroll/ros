use bootloader_api::info::{FrameBufferInfo, PixelFormat};
use core::fmt::{self, Write};
use noto_sans_mono_bitmap::{
    FontWeight, RasterHeight, RasterizedChar, get_raster, get_raster_width,
};
use spin::Mutex;

const WEIGHT: FontWeight = FontWeight::Regular;
const HEIGHT: RasterHeight = RasterHeight::Size16;
const CHAR_WIDTH: usize = get_raster_width(WEIGHT, HEIGHT);
const LINE_HEIGHT: usize = HEIGHT.val() + 2;
const BORDER: usize = 3;

pub struct Writer {
    buf: &'static mut [u8],
    info: FrameBufferInfo,
    x: usize,
    y: usize,
}

impl Writer {
    fn new(buf: &'static mut [u8], info: FrameBufferInfo) -> Self {
        let mut w = Self {
            buf,
            info,
            x: BORDER,
            y: BORDER,
        };
        w.clear();
        w
    }

    pub fn clear(&mut self) {
        self.buf.fill(0);
        self.x = BORDER;
        self.y = BORDER;
    }
}

pub static WRITER: Mutex<Option<Writer>> = Mutex::new(None);

pub fn init(buf: &'static mut [u8], info: FrameBufferInfo) {
    *WRITER.lock() = Some(Writer::new(buf, info));
}
