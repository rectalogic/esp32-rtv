use esp_idf_svc::sys::*;
use std::{ffi::c_int, iter::Peekable};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MjpegError {
    #[error("JPEG decoder error")]
    JpegError(i32),
    #[error("allocation failed")]
    AllocFailed,
    #[error("MJPEG stream exhausted")]
    StreamExhausted,
}

impl From<i32> for MjpegError {
    fn from(value: i32) -> Self {
        MjpegError::JpegError(value)
    }
}

pub struct MjpegDecoder<'a, I>
where
    I: Iterator<Item = &'a [u8]>,
{
    handle: jpeg_dec_handle_t,
    mcu_buffer: McuBuffer,
    mcu_count: c_int,
    mjpeg: Peekable<I>,
}

pub struct McuBuffer {
    ptr: *mut u8,
    size: usize,
}

impl McuBuffer {
    fn new(size: usize) -> Result<Self, MjpegError> {
        let ptr = unsafe { jpeg_calloc_align(size, 16) };
        if ptr.is_null() {
            return Err(MjpegError::AllocFailed);
        }
        Ok(Self {
            ptr: ptr as *mut u8,
            size,
        })
    }
}

impl Drop for McuBuffer {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                jpeg_free_align(self.ptr as *mut _);
            }
        }
    }
}

impl<'a, I> MjpegDecoder<'a, I>
where
    I: Iterator<Item = &'a [u8]>,
{
    pub fn open(mjpeg: I) -> Result<Self, MjpegError> {
        let config = jpeg_dec_config_t {
            output_type: jpeg_pixel_format_t_JPEG_PIXEL_FORMAT_RGB565_LE,
            block_enable: true,
            ..Default::default()
        };

        let mut handle: jpeg_dec_handle_t = std::ptr::null_mut();
        let ret = unsafe {
            jpeg_dec_open(
                &config as *const jpeg_dec_config_t as *mut jpeg_dec_config_t,
                &mut handle,
            )
        };
        if ret != jpeg_error_t_JPEG_ERR_OK {
            return Err(ret.into());
        }

        let mut mjpeg = mjpeg.peekable();
        let jpeg_data = *mjpeg.peek().ok_or(MjpegError::StreamExhausted)?;
        let mut jpeg_io = jpeg_dec_io_t {
            inbuf: jpeg_data.as_ptr() as *mut u8,
            inbuf_len: jpeg_data.len() as i32,
            ..Default::default()
        };

        let mut header_info = jpeg_dec_header_info_t::default();
        let ret = unsafe {
            jpeg_dec_parse_header(
                handle,
                &mut jpeg_io as *mut jpeg_dec_io_t,
                &mut header_info as *mut jpeg_dec_header_info_t,
            )
        };
        if ret != jpeg_error_t_JPEG_ERR_OK {
            return Err(ret.into());
        }

        let mut mcu_len: c_int = 0;
        let ret = unsafe { jpeg_dec_get_outbuf_len(handle, &mut mcu_len as *mut c_int) };
        if ret != jpeg_error_t_JPEG_ERR_OK || mcu_len == 0 {
            return Err(ret.into());
        }
        let mcu_buffer = McuBuffer::new(mcu_len as usize)?;

        let mut mcu_count: c_int = 0;
        let ret = unsafe { jpeg_dec_get_process_count(handle, &mut mcu_count as *mut c_int) };
        if ret != jpeg_error_t_JPEG_ERR_OK || mcu_count == 0 {
            return Err(ret.into());
        }

        Ok(Self {
            handle,
            mcu_buffer,
            mcu_count,
            mjpeg,
        })
    }

    // Pass a callback to receive each McuBuffer
    pub fn decode(&mut self) -> Result<(), MjpegError> {
        let jpeg_data = self.mjpeg.next().ok_or(MjpegError::StreamExhausted)?;
        let mut jpeg_io = jpeg_dec_io_t {
            inbuf: jpeg_data.as_ptr() as *mut u8,
            inbuf_len: jpeg_data.len() as i32,
            outbuf: self.mcu_buffer.ptr,
            out_size: self.mcu_buffer.size as i32,
            ..Default::default()
        };
        for _ in 0..self.mcu_count {
            let ret = unsafe { jpeg_dec_process(self.handle, &mut jpeg_io as *mut jpeg_dec_io_t) };
            if ret != jpeg_error_t_JPEG_ERR_OK {
                return Err(ret.into());
            }
        }
        Ok(())
    }
}

impl<'a, I> Drop for MjpegDecoder<'a, I>
where
    I: Iterator<Item = &'a [u8]>,
{
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                jpeg_dec_close(self.handle);
            }
        }
    }
}
