#![no_std]

extern crate embedded_hal;

use embedded_hal::serial::{Read, Write};
use image_receiver::ImageReceiver;

mod crc;
mod image_receiver;

pub enum WriteError
{
    NoData,
    AddressOutOfRange
}

pub enum ReadError
{
    AddressOutOfRange,
    EndAddressOutOfRange,
    ReadFailed
}

#[derive(Debug, PartialEq)]
pub enum UpdateEncoding
{
    Raw,
    LZMA
}
#[repr(C)]
pub struct bin_info
{
    magic: [u8;5],
    struct_ver: u8,
    app_start: usize,
    app_len: usize,
    checksum: u32
}


#[repr(C)]
struct update_info
{
    magic: [u8;5],
    struct_ver: u8,
    update_start: usize,
    update_len: usize,
    target_adress: usize,
    update_encoding: UpdateEncoding,
    checksum: u32
}

pub trait Flasher
{
    fn write(&mut self, destination: usize, data: &[u8]) -> Result<(), WriteError>;
    fn read(&self, source_address: usize, destination: &mut[u8]) -> Result<usize, ReadError>;
    fn flush(&mut self);
}

fn load_info_struct_from_address<T, F>(address: usize, flasher: &F) -> Result<T, ReadError>
    where F: Flasher, T: Sized
{
    unsafe 
    {
        let mut result: T = core::mem::zeroed();
        let num_bytes = core::mem::size_of::<T>();
        let data_slice = core::slice::from_raw_parts_mut((&mut result as *mut T) as *mut u8, num_bytes);

        let bytes_read = flasher.read( address, &mut *data_slice)?;
        if bytes_read == num_bytes
        {
            return Ok(result);
        }
    }
    Err(ReadError::ReadFailed)
}

fn on_error() -> !
{
    loop{}
}

fn check_update<T>(data: &update_info, flasher: &T) -> bool
    where T: Flasher
{
    let magic = b"MUUPD";

    if *magic != data.magic
    {
        return false;
    }

    if data.struct_ver != 1
    {
        return false;
    }

    if data.update_encoding != UpdateEncoding::Raw
    {
        return false;
    }

    return crc::check_crc(data.update_start, data.update_len, data.checksum, flasher);

}

fn check_binary<T>(data: &bin_info, flasher: &T) -> bool
    where T: Flasher
{
    let magic = b"MUBIN";

    if *magic != data.magic
    {
        return false;
    }

    if data.struct_ver != 1
    {
        return false;
    }

    return crc::check_crc(data.app_start, data.app_len, data.checksum, flasher);

}

fn install_binary<T>(data: &update_info, flasher: &mut T) -> bool
    where T: Flasher
{
    const BUF_SIZE: usize = 64;
    let mut buff: [u8; BUF_SIZE] = unsafe {core::mem::zeroed()};
    let mut bytes_left = data.update_len as usize;
    let mut bytes_written: usize = 0;
    while bytes_left > 0
    {
        if let Ok(result) = flasher.read(data.update_start + bytes_written, &mut buff)
        {
            let dst_slice = &buff[0..result];
            if let Ok(()) = flasher.write(data.target_adress + bytes_written, dst_slice)
            {
                bytes_left = bytes_left - result;
                bytes_written = bytes_written + result;
            }
            else
            {
                // Failed to write.
            }
        }
        else
        {
            // failed to read!
        }
        
    }
    
    flasher.flush();

    return crc::check_crc(data.target_adress, data.update_len, data.checksum, flasher);
}

fn receive_binary<T: Flasher, U: Read<u8> + Write<u8> >(flasher: T, uart: U) -> !
{    
    let rec = ImageReceiver::new(flasher, uart);
    rec.execute();
}


pub fn muload_main<T, U: Read<u8> + Write<u8> >(update_info_address: usize, bin_info_address: usize, mut flasher: T, mut uart: U)
    where T: Flasher 
{
    // first steps first: Send out a notification
    // that we are available and wait up to 100 ms for a download request.

    // Assumption: Lowlevel init has been done by some other piece of code,
    // we can immediately check if we have a new binary
    if let Ok(update_info) = load_info_struct_from_address::<update_info, T>(update_info_address, &flasher)
    {
        if check_update(&update_info, &flasher)
        {
            if !install_binary(&update_info, &mut flasher)
            {
                // Installation failed. This is basically the worst case as
                // we now destroyed the installed image with a halfbaked version
                // of the previous image. We can't do much here. Note that this
                // issue can only arise if the actual installation failed as
                // a faulty image would have been caught by check_update.
                on_error();
            }
        }
    }

    // // At this point: either a binary was installed... or not. We don't care for now,
    // // but attempt to launch the actually installed binary if that is good:
    if let Ok(binary_info) = load_info_struct_from_address::<bin_info, T>(bin_info_address, &flasher)
    {
        if !check_binary(&binary_info, &flasher)
        {
            // Note that we assume that the app binary will setup its own stack and the likes
            // so basically: after we call app_start everything will be setup by the cstart routine (or similar)
            // of the binary.
            // let app_start = unsafe {(binary_info.app_start as *const usize) as extern "C" fn() -> !};
            // app_start();
        }
        loop{}
    }
    else
    {
        // Nothing bootable available - we stay in bootmode and wait until someone sends us
        // a binary via usart
        receive_binary(flasher, uart);
        // after we received the binary we just reboot. We'll endup in this function again
        // with a hopefully wellformed update_info which can be installed and booted.        
    }
}