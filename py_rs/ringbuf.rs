//! rewrite of py/ringbuf.h + py/ringbuf.c
// symmetry: done

/// Byte ring buffer (`ringbuf_t`). Capacity keeps one empty slot.
#[derive(Debug)]
pub struct Ringbuf {
    pub buf: Vec<u8>,
    pub iget: u16,
    pub iput: u16,
}

impl Ringbuf {
    pub fn new(size: usize) -> Self {
        assert!(size > 1 && size <= u16::MAX as usize);
        Self {
            buf: vec![0; size],
            iget: 0,
            iput: 0,
        }
    }

    pub fn size(&self) -> u16 {
        self.buf.len() as u16
    }

    pub fn reset(&mut self) {
        self.iget = 0;
        self.iput = 0;
    }

    pub fn get(&mut self) -> i32 {
        if self.iget == self.iput {
            return -1;
        }
        let v = self.buf[self.iget as usize];
        self.iget += 1;
        if self.iget >= self.size() {
            self.iget = 0;
        }
        v as i32
    }

    pub fn peek(&self) -> i32 {
        if self.iget == self.iput {
            return -1;
        }
        self.buf[self.iget as usize] as i32
    }

    pub fn put(&mut self, v: u8) -> i32 {
        let mut iput_new = self.iput as u32 + 1;
        if iput_new >= self.size() as u32 {
            iput_new = 0;
        }
        if iput_new == self.iget as u32 {
            return -1;
        }
        self.buf[self.iput as usize] = v;
        self.iput = iput_new as u16;
        0
    }

    pub fn free(&self) -> usize {
        let size = self.size() as usize;
        (size + self.iget as usize - self.iput as usize - 1) % size
    }

    pub fn avail(&self) -> usize {
        let size = self.size() as usize;
        (size + self.iput as usize - self.iget as usize) % size
    }

    fn memcpy_get_internal(&mut self, data: &mut [u8]) {
        let size = self.size() as u32;
        let mut iget = self.iget as u32;
        let data_len = data.len() as u32;
        let iget_a = (iget + data_len) % size;
        let mut offset = 0usize;
        if iget_a < iget {
            let n = (size - iget) as usize;
            data[offset..offset + n].copy_from_slice(&self.buf[iget as usize..]);
            offset += n;
            iget = 0;
        }
        let n = (iget_a - iget) as usize;
        data[offset..offset + n].copy_from_slice(&self.buf[iget as usize..iget as usize + n]);
        self.iget = iget_a as u16;
    }

    fn memcpy_put_internal(&mut self, data: &[u8]) {
        let size = self.size() as u32;
        let mut iput = self.iput as u32;
        let data_len = data.len() as u32;
        let iput_a = (iput + data_len) % size;
        let mut offset = 0usize;
        if iput_a < iput {
            let n = (size - iput) as usize;
            self.buf[iput as usize..].copy_from_slice(&data[offset..offset + n]);
            offset += n;
            iput = 0;
        }
        let n = (iput_a - iput) as usize;
        self.buf[iput as usize..iput as usize + n].copy_from_slice(&data[offset..offset + n]);
        self.iput = iput_a as u16;
    }

    /// Big-endian 16-bit get (`ringbuf_get16`).
    pub fn get16(&mut self) -> i32 {
        let v = self.peek16();
        if v == -1 {
            return v;
        }
        self.iget += 2;
        if self.iget >= self.size() {
            self.iget -= self.size();
        }
        v
    }

    pub fn peek16(&self) -> i32 {
        if self.iget == self.iput {
            return -1;
        }
        let mut iget_a = self.iget as u32 + 1;
        if iget_a == self.size() as u32 {
            iget_a = 0;
        }
        if iget_a == self.iput as u32 {
            return -1;
        }
        ((self.buf[self.iget as usize] as i32) << 8) | (self.buf[iget_a as usize] as i32)
    }

    pub fn put16(&mut self, v: u16) -> i32 {
        let mut iput_a = self.iput as u32 + 1;
        if iput_a == self.size() as u32 {
            iput_a = 0;
        }
        if iput_a == self.iget as u32 {
            return -1;
        }
        let mut iput_b = iput_a + 1;
        if iput_b == self.size() as u32 {
            iput_b = 0;
        }
        if iput_b == self.iget as u32 {
            return -1;
        }
        self.buf[self.iput as usize] = ((v >> 8) & 0xff) as u8;
        self.buf[iput_a as usize] = (v & 0xff) as u8;
        self.iput = iput_b as u16;
        0
    }

    /// 0 success, -1 not enough data, -2 request larger than buffer.
    pub fn get_bytes(&mut self, data: &mut [u8]) -> i32 {
        if self.avail() < data.len() {
            return if self.size() as usize <= data.len() {
                -2
            } else {
                -1
            };
        }
        self.memcpy_get_internal(data);
        0
    }

    pub fn put_bytes(&mut self, data: &[u8]) -> i32 {
        if self.free() < data.len() {
            return if self.size() as usize <= data.len() {
                -2
            } else {
                -1
            };
        }
        self.memcpy_put_internal(data);
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_put_get_and_16() {
        let mut r = Ringbuf::new(8);
        assert_eq!(r.put(0x12), 0);
        assert_eq!(r.put(0x34), 0);
        assert_eq!(r.get(), 0x12);
        assert_eq!(r.get(), 0x34);
        assert_eq!(r.put16(0xabcd), 0);
        assert_eq!(r.peek16(), 0xabcd);
        assert_eq!(r.get16(), 0xabcd);
        // Wrap around the buffer with a multi-byte transfer.
        for b in 0..5u8 {
            assert_eq!(r.put(b), 0);
        }
        let mut out = [0u8; 5];
        assert_eq!(r.get_bytes(&mut out), 0);
        assert_eq!(out, [0, 1, 2, 3, 4]);
    }
}
