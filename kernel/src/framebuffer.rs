use crate::sync::IrqSpinLock;
use bootloader_api::info::{FrameBufferInfo, PixelFormat};
use noto_sans_mono_bitmap::{
    FontWeight, RasterHeight, RasterizedChar, get_raster, get_raster_width,
};

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

    fn newline(&mut self) {
        self.x = BORDER;
        self.y += LINE_HEIGHT;
        if self.y + LINE_HEIGHT > self.info.height {
            self.scroll();
        }
    }

    fn scroll(&mut self) {
        let row_bytes = self.info.stride * self.info.bytes_per_pixel;
        let shift = LINE_HEIGHT * row_bytes;
        let len = self.buf.len();
        self.buf.copy_within(shift..len, 0);
        self.buf[len - shift..].fill(0);
        self.y -= LINE_HEIGHT;
    }

    fn write_char(&mut self, character: char) {
        match character {
            '\n' => self.newline(),
            '\r' => self.x = BORDER,
            character => {
                if self.x + CHAR_WIDTH > self.info.width {
                    self.newline();
                }
                let glyph = get_raster(character, WEIGHT, HEIGHT)
                    .or_else(|| get_raster('?', WEIGHT, HEIGHT))
                    .expect("'?' is always in the font");
                self.draw(&glyph);
                self.x += glyph.width();
            }
        }
    }

    fn draw(&mut self, glyph: &RasterizedChar) {
        for (dy, row) in glyph.raster().iter().enumerate() {
            for (dx, &intensity) in row.iter().enumerate() {
                self.put_pixel(self.x + dx, self.y + dy, intensity);
            }
        }
    }

    fn put_pixel(&mut self, x: usize, y: usize, intensity: u8) {
        let bytes_per_pixel = self.info.bytes_per_pixel;
        let offset = (y * self.info.stride + x) * bytes_per_pixel;
        let px = match self.info.pixel_format {
            PixelFormat::Rgb => [intensity, intensity, intensity / 4, 0],
            PixelFormat::Bgr => [intensity / 4, intensity, intensity, 0],
            PixelFormat::U8 => [intensity, 0, 0, 0],
            _ => todo!("Properly return an error"),
        };
        self.buf[offset..offset + bytes_per_pixel].copy_from_slice(&px[..bytes_per_pixel])
    }
}

impl core::fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        s.chars().for_each(|character| self.write_char(character));
        Ok(())
    }
}

pub static WRITER: IrqSpinLock<Option<Writer>> = IrqSpinLock::new(None);

pub fn init(buf: &'static mut [u8], info: FrameBufferInfo) {
    *WRITER.lock() = Some(Writer::new(buf, info));
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::framebuffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => (crate::print!("\n"));
    ($($arg:tt)*) => (crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    if let Some(writer) = WRITER.lock().as_mut() {
        writer.write_fmt(args).unwrap();
    }
}
