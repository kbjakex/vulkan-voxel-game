
#[inline]
pub fn f32_to_fixed(f: f32, fractional_bits: u32) -> u32 {
    (f * (1 << fractional_bits) as f32).round() as i32 as u32
}

#[inline]
pub fn fixed_to_f32(fp: u32, fractional_bits: u32) -> f32 {
    (fp as i32) as f32 / (1 << fractional_bits) as f32
}

#[inline]
pub fn round_to_frac_bits(f: f32, fractional_bits: u32) -> f32 {
    fixed_to_f32(f32_to_fixed(f, fractional_bits), fractional_bits)
}


pub struct ByteReader<'a> {
    src: &'a [u8],
    pos: usize
}

#[allow(unused)]
impl<'a> ByteReader<'a> {
    pub fn new(src: &'a [u8]) -> Self {
        Self {
            src,
            pos: 0
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.src[self.pos..]
    }

    pub fn mark_start(&mut self) {
        self.src = &self.src[self.pos..];
        self.reset();
    }

    pub fn reset(&mut self) {
        self.pos = 0;
    }
    
    pub fn total_src_size(&self) -> usize {
        self.src.len() 
    }

    pub fn bytes_remaining(&self) -> usize {
        self.src.len() - self.pos
    }

    pub fn bytes_read(&self) -> usize {
        self.pos
    }

    pub fn has_n_more(&self, n: usize) -> bool {
        self.bytes_remaining() >= n
    }

    pub fn skip(&mut self, n: usize) {
        self.pos += n;
    }

    pub fn back(&mut self, n: usize) {
        self.pos -= n;
    }

    pub fn read(&mut self, dst: &mut [u8]) {
        dst.copy_from_slice(unsafe {self.src.get_unchecked(self.pos..self.pos + dst.len()) });
        self.pos += dst.len();
    }

    pub fn read_u8(&mut self) -> u8 {
        let p = self.pos;
        self.pos += 1;
        u8::from_le_bytes(unsafe {[
            *self.src.get_unchecked(p)
        ]})
    }

    pub fn read_u16(&mut self) -> u16 {
        let p = self.pos;
        self.pos += 2;
        u16::from_le_bytes(unsafe {[
            *self.src.get_unchecked(p),
            *self.src.get_unchecked(p + 1)
        ]})
    }

    pub fn read_u32(&mut self) -> u32 {
        let p = self.pos;
        self.pos += 4;
        u32::from_le_bytes(unsafe {[
            *self.src.get_unchecked(p),
            *self.src.get_unchecked(p + 1),
            *self.src.get_unchecked(p + 2),
            *self.src.get_unchecked(p + 3)
        ]})
    }

    pub fn read_u64(&mut self) -> u64 {
        let p = self.pos;
        self.pos += 8;
        u64::from_le_bytes(unsafe {[
            *self.src.get_unchecked(p),
            *self.src.get_unchecked(p + 1),
            *self.src.get_unchecked(p + 2),
            *self.src.get_unchecked(p + 3),
            *self.src.get_unchecked(p + 4),
            *self.src.get_unchecked(p + 5),
            *self.src.get_unchecked(p + 6),
            *self.src.get_unchecked(p + 7)
        ]})
    }

    pub fn read_i8(&mut self) -> i8 {
        let p = self.pos;
        self.pos += 1;
        i8::from_le_bytes(unsafe {[
            *self.src.get_unchecked(p)
        ]})
    }

    pub fn read_i16(&mut self) -> i16 {
        let p = self.pos;
        self.pos += 2;
        i16::from_le_bytes(unsafe {[
            *self.src.get_unchecked(p),
            *self.src.get_unchecked(p + 1)
        ]})
    }

    pub fn read_i32(&mut self) -> i32 {
        let p = self.pos;
        self.pos += 4;
        i32::from_le_bytes(unsafe {[
            *self.src.get_unchecked(p),
            *self.src.get_unchecked(p + 1),
            *self.src.get_unchecked(p + 2),
            *self.src.get_unchecked(p + 3)
        ]})
    }

    pub fn read_i64(&mut self) -> i64 {
        let p = self.pos;
        self.pos += 8;
        i64::from_le_bytes(unsafe {[
            *self.src.get_unchecked(p),
            *self.src.get_unchecked(p + 1),
            *self.src.get_unchecked(p + 2),
            *self.src.get_unchecked(p + 3),
            *self.src.get_unchecked(p + 4),
            *self.src.get_unchecked(p + 5),
            *self.src.get_unchecked(p + 6),
            *self.src.get_unchecked(p + 7)
        ]})
    }

    pub fn read_f32(&mut self) -> f32 {
        f32::from_bits(self.read_u32())
    }

    pub fn read_f64(&mut self) -> f64 {
        f64::from_bits(self.read_u64())
    }

    pub fn read_str(&mut self, len: usize) -> &'a str {
        let pos = self.pos;
        self.pos += len;
        unsafe { std::str::from_utf8_unchecked(&self.src[pos..pos + len]) }
    }

    pub fn read_bool(&mut self) -> bool {
        self.read_u8() != 0
    }
}


pub struct ByteWriter<'a> {
    dst: &'a mut [u8],
    pos: u32
}

#[allow(unused)]
impl<'a> ByteWriter<'a> {
    pub fn new(dst: &'a mut [u8]) -> Self {
        Self {
            dst,
            pos: 0
        }
    }

    pub fn new_for_message(dst: &'a mut [u8]) -> Self {
        Self {
            dst,
            pos: 2,
        }
    }

    pub fn bytes_written(&self) -> usize {
        self.pos as _
    }

    pub fn space_remaining(&self) -> usize {
        self.dst.len() - self.pos as usize
    } 

    pub fn write(&mut self, src: &[u8]) {
        debug_assert!(src.len() <= self.dst.len() - self.pos as usize);

        unsafe {self.dst.get_unchecked_mut(self.pos as usize..self.pos as usize + src.len()) }.copy_from_slice(src);
        self.pos += src.len() as u32;
    }

    pub fn write_message_len(&mut self) {
        let len = (self.bytes_written() as u16).saturating_sub(2);
        let off = ByteWriter::new(self.dst).write_varint15_r(len);
        let (_, new) = std::mem::take(&mut self.dst).split_at_mut(off);
        self.dst = new;
        self.pos -= off as u32;
    }

    // write right-aligned 15-bit varint
    pub fn write_varint15_r(&mut self, mut x: u16) -> usize {
        debug_assert!(x < 32768, "value {x} too large, varint15 needs a control bit");

        if x > 127 {
            x = (x & 127) | ((x & !127) << 1) | 128;
            self.write_u16(x);
            0
        } else {
            self.write_u8(7); // skip one
            self.write_u8(x as u8);
            1
        }
    }

    pub fn write_u8(&mut self, x: u8) {
        debug_assert!(self.dst.len() - self.pos as usize >= 1);

        let bytes = u8::to_le_bytes(x);
        let p = self.pos as usize;
        self.pos += 1;
        unsafe {
            *self.dst.get_unchecked_mut(p) = *bytes.get_unchecked(0);
        }
    }

    pub fn write_u16(&mut self, x: u16) {
        debug_assert!(self.dst.len() - self.pos as usize >= 2);

        let bytes = u16::to_le_bytes(x);
        let p = self.pos as usize;
        self.pos += 2;
        unsafe {
            *self.dst.get_unchecked_mut(p) = *bytes.get_unchecked(0);
            *self.dst.get_unchecked_mut(p + 1) = *bytes.get_unchecked(1);
        }
    }

    pub fn write_u32(&mut self, x: u32) {
        debug_assert!(self.dst.len() - self.pos as usize >= 4);

        let bytes = u32::to_le_bytes(x);
        let p = self.pos as usize;
        self.pos += 4;
        unsafe {
            *self.dst.get_unchecked_mut(p) = *bytes.get_unchecked(0);
            *self.dst.get_unchecked_mut(p + 1) = *bytes.get_unchecked(1);
            *self.dst.get_unchecked_mut(p + 2) = *bytes.get_unchecked(2);
            *self.dst.get_unchecked_mut(p + 3) = *bytes.get_unchecked(3);
        }
    }

    pub fn write_u64(&mut self, x: u64) {
        debug_assert!(self.dst.len() - self.pos as usize >= 8);

        let bytes = u64::to_le_bytes(x);
        let p = self.pos as usize;
        self.pos += 8;
        unsafe {
            *self.dst.get_unchecked_mut(p) = *bytes.get_unchecked(0);
            *self.dst.get_unchecked_mut(p + 1) = *bytes.get_unchecked(1);
            *self.dst.get_unchecked_mut(p + 2) = *bytes.get_unchecked(2);
            *self.dst.get_unchecked_mut(p + 3) = *bytes.get_unchecked(3);
            *self.dst.get_unchecked_mut(p + 4) = *bytes.get_unchecked(4);
            *self.dst.get_unchecked_mut(p + 5) = *bytes.get_unchecked(5);
            *self.dst.get_unchecked_mut(p + 6) = *bytes.get_unchecked(6);
            *self.dst.get_unchecked_mut(p + 7) = *bytes.get_unchecked(7);
        }
    }

    pub fn write_i8(&mut self, x: i8) {
        debug_assert!(self.dst.len() - self.pos as usize >= 1);

        let bytes = i8::to_le_bytes(x);
        let p = self.pos as usize;
        self.pos += 1;
        unsafe {
            *self.dst.get_unchecked_mut(p) = *bytes.get_unchecked(0);
        }
    }

    pub fn write_i16(&mut self, x: i16) {
        debug_assert!(self.dst.len() - self.pos as usize >= 2);

        let bytes = i16::to_le_bytes(x);
        let p = self.pos as usize;
        self.pos += 2;
        unsafe {
            *self.dst.get_unchecked_mut(p) = *bytes.get_unchecked(0);
            *self.dst.get_unchecked_mut(p + 1) = *bytes.get_unchecked(1);
        }
    }

    pub fn write_i32(&mut self, x: i32) {
        debug_assert!(self.dst.len() - self.pos as usize >= 4);

        let bytes = i32::to_le_bytes(x);
        let p = self.pos as usize;
        self.pos += 4;
        unsafe {
            *self.dst.get_unchecked_mut(p) = *bytes.get_unchecked(0);
            *self.dst.get_unchecked_mut(p + 1) = *bytes.get_unchecked(1);
            *self.dst.get_unchecked_mut(p + 2) = *bytes.get_unchecked(2);
            *self.dst.get_unchecked_mut(p + 3) = *bytes.get_unchecked(3);
        }
    }

    pub fn write_i64(&mut self, x: i64) {
        debug_assert!(self.dst.len() - self.pos as usize >= 8);

        let bytes = i64::to_le_bytes(x);
        let p = self.pos as usize;
        self.pos += 8;
        unsafe {
            *self.dst.get_unchecked_mut(p) = *bytes.get_unchecked(0);
            *self.dst.get_unchecked_mut(p + 1) = *bytes.get_unchecked(1);
            *self.dst.get_unchecked_mut(p + 2) = *bytes.get_unchecked(2);
            *self.dst.get_unchecked_mut(p + 3) = *bytes.get_unchecked(3);
            *self.dst.get_unchecked_mut(p + 4) = *bytes.get_unchecked(4);
            *self.dst.get_unchecked_mut(p + 5) = *bytes.get_unchecked(5);
            *self.dst.get_unchecked_mut(p + 6) = *bytes.get_unchecked(6);
            *self.dst.get_unchecked_mut(p + 7) = *bytes.get_unchecked(7);
        }
    }

    pub fn write_f32(&mut self, x: f32) {
        self.write_u32(x.to_bits());
    }

    pub fn write_f64(&mut self, x: f64) {
        self.write_u64(x.to_bits());
    }

    pub fn write_str(&mut self, x: &str) {
        self.write_u16(x.len() as u16);
        self.write(x.as_bytes());
    }

    pub fn write_bool(&mut self, x: bool) {
        self.write_u8(x as u8);
    }

    pub fn bytes(&self) -> &[u8] {
        &self.dst[..self.pos as usize]
    }
}

pub struct BitReader<'a> {
    current: u64,
    bits_left: u32,
    buf_pos: usize,
    buf: &'a [u8],
}

// Reading
impl<'a> BitReader<'a> {
    #[inline]
    pub fn new(buf: &'a [u8]) -> Self {
        let mut ret = Self { buf, bits_left: 64, buf_pos: 0, current: 0 };

        ret.current = ((ret.read() as u64)) | ((ret.read() as u64) << 32);
        ret
    }

    // Reads the next chunk of 32 bits from the buffer, or zero bits
    // if past the end
    #[inline]
    fn read(&mut self) -> u32 {
        let left = 4.min(self.buf.len() - self.buf_pos);
        let mut bytes = [0u8; 4];
        bytes[..left].copy_from_slice(&self.buf[self.buf_pos .. self.buf_pos + left]);
        self.buf_pos += left;
        return u32::from_le_bytes(bytes);
    }

    #[inline]
    pub fn uint(&mut self, num_bits: u32) -> u32 {
        debug_assert!(num_bits <= 32);

        let result = self.current & !(!0 << num_bits);
        
        self.bits_left -= num_bits;
        self.current >>= num_bits;
        
        if self.bits_left < 32 {
            self.current |= (self.read() as u64) << self.bits_left;
            self.bits_left += 32;
        }

        result as u32
    }

    #[inline]
    pub fn int(&mut self, num_bits: u32) -> i32 {
        let u = self.uint(num_bits);
        (u as i64 - (1 << (num_bits-1))) as i32
    }

    #[inline]
    pub fn bool(&mut self) -> bool {
        self.uint(1) != 0
    }
}

pub struct BitWriter<'a> {
    current: u64,
    bit_pos: u32,
    buf: &'a mut [u8],
    bits_written: usize,
}

// Writing
impl<'a> BitWriter<'a> {
    #[inline]
    pub fn new(buf: &'a mut [u8]) -> Self {
        debug_assert!(buf.len() % 4 == 0, "Buffer length must be a multiple of 4 to avoid surprises");
        Self { buf, bit_pos: 0, current: 0, bits_written: 0 }
    }

    #[inline]
    fn write(& mut self, val: u32) {
        if self.buf.len() >= 4 {
            let (dst, rest) = std::mem::take(&mut self.buf).split_at_mut(4);
            dst.copy_from_slice(&val.to_le_bytes());
            self.buf = rest;
            self.bits_written += 32;
        } else {
            debug_assert!(false, "BitWriter: write() out of bounds");
        }
    }

    #[inline]
    pub fn uint(&mut self, value: u32, num_bits: u32) -> u32 {
        debug_assert!(num_bits <= 32);
        //debug_assert_eq!(0, value as u64 >> num_bits);

        self.current |= (value as u64) << self.bit_pos;
        self.bit_pos += num_bits;

        if self.bit_pos >= 32 {
            self.write(self.current as u32);
            self.current >>= 32;
            self.bit_pos -= 32;
        }

        value
    }

    #[inline]
    pub fn int(&mut self, value: i32, num_bits: u32) -> i32 {
        self.uint(((value as u32).wrapping_add(1 << (num_bits-1))) & !(!0 << num_bits), num_bits);
        value
    }

    #[inline]
    pub fn bool(&mut self, b: bool) -> bool {
        self.uint(b as u32, 1);
        b
    }

    #[inline]
    pub fn flush_partials(&mut self) {
        if self.bit_pos == 0 {
            return;
        }
        self.write(self.current as u32 & !(0xFFFF_FFFF << self.bit_pos));
        self.bits_written -= 32; // write() assumes all 32 bits are used
        self.bits_written += self.bit_pos as usize;
    }

    #[inline]
    pub fn bits_written(&self) -> usize {
        self.bits_written
    }

    #[inline]
    pub fn compute_bytes_written(&self) -> usize {
        (self.bits_written + 7) / 8 
    }
}

mod tests {
    #[test]
    pub fn test_roundtrip() {
        let mut buf = [0u8; 28];
        let mut writer = super::BitWriter::new(&mut buf);

        writer.uint(0x123, 12);
        writer.bool(true);
        writer.uint(0x4D, 7);
        writer.uint(0xFFFF_FFFF, 32);
        writer.uint(0xAAAA, 16);
        writer.int(-12345678, 28);
        writer.int(134217727, 28);
        writer.int(0, 28);
        writer.int(-134217728, 28);

        println!("{}", writer.compute_bytes_written());
        
        writer.uint(0xAB, 8);
        writer.uint(0xCD, 8);

        writer.flush_partials();

        println!("{}", writer.compute_bytes_written());

        assert_eq!(writer.bits_written(), 196);
        assert_eq!(writer.compute_bytes_written(), 25);

        let buf = &buf[..25];

        let mut reader = super::BitReader::new(buf);
        assert_eq!(reader.uint(12), 0x123);
        assert_eq!(reader.bool(), true);
        assert_eq!(reader.uint(7), 0x4D);
        assert_eq!(reader.uint(32), 0xFFFF_FFFF);
        assert_eq!(reader.uint(16), 0xAAAA);
        assert_eq!(reader.int(28), -12345678);
        assert_eq!(reader.int(28), 134217727);
        assert_eq!(reader.int(28), 0);
        assert_eq!(reader.int(28), -134217728);


        // Little-endian, so 0xAB, 0xCD => 0xCD_AB
        assert_eq!(reader.uint(16), 0xCDAB);

        // Any reads past the end are zeros
        assert_eq!(reader.uint(32), 0);
        assert_eq!(reader.uint(32), 0);
        assert_eq!(reader.uint(32), 0);
        assert_eq!(reader.uint(32), 0);
        assert_eq!(reader.uint(32), 0);
    }
}