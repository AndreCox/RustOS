#[repr(C)]
pub struct PsfHeader {
    magic: u32,
    version: u32,
    header_size: u32,
    flags: u32,
    length: u32,
    char_size: u32,
    pub height: u32,
    pub width: u32,
}

pub struct Font {
    data: &'static [u8],
    pub header: &'static PsfHeader,
}

#[repr(align(16))]
pub struct AlignedData<T: ?Sized>(T);

pub static FONT_DATA: &AlignedData<[u8]> =
    &AlignedData(*include_bytes!("../../assets/fonts/sanserif.psf"));

impl Font {
    pub fn new(data: &'static AlignedData<[u8]>) -> Self {
        unsafe {
            let header = &*(data.0.as_ptr() as *const PsfHeader);
            Self {
                data: &data.0,
                header,
            }
        }
    }

    pub fn get_glyph(&self, c: char) -> &[u8] {
        let codepoint = if (c as u32) < self.header.length {
            c as u32
        } else {
            0
        };
        let offset = self.header.header_size as usize
            + (codepoint as usize * self.header.char_size as usize);
        &self.data[offset..offset + self.header.char_size as usize]
    }
}
