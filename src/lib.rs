#![no_std]

extern crate embedded_hal;
extern crate nb;

use embedded_hal::serial::{Read, Write};
use image_receiver::ImageReceiver;

mod crc;
mod image_receiver;
mod image_installer;
mod image_launcher;

#[cfg(test)]
mod testhelpers;

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
    checksum: usize
}


#[repr(C)]
pub struct update_info
{
    magic: [u8;5],
    struct_ver: u8,
    update_start: usize,
    update_len: usize,
    target_adress: usize,
    //update_encoding: UpdateEncoding,
    checksum: usize
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

// fn any_from_byte_array<T>(data: &[u8]) -> Result<T, ReadError>
//     where T: Sized
// {
//     unsafe 
//     {
//         let mut result: T = core::mem::zeroed();
//         let num_bytes = core::mem::size_of::<T>();
//         let data_slice = core::slice::from_raw_parts_mut((&mut result as *mut T) as *mut u8, num_bytes);
//         //data_slice.write(data);
//         core::ptr::copy(data, data_slice, num_bytes);
//         // if data.len() != num_bytes
//         // {
//              return Err(ReadError::AddressOutOfRange);
//         // }

//         // result = unsafe { *data as T };
//         // return Ok(result);
//     }
// }

fn on_error() -> !
{
    loop{}
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
        if image_installer::check_update(&update_info, &flasher)
        {
            if !image_installer::install_binary(&update_info, &mut flasher)
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
        if !image_launcher::check_binary(&binary_info, &flasher)
        {
            // Note that we assume that the app binary will setup its own stack and the likes
            // so basically: after we call app_start everything will be setup by the cstart routine (or similar)
            // of the binary.
            image_launcher::launch_binary(binary_info);
        }
        // we should never get here!
        on_error();
    }
    else
    {
        // Nothing bootable available - we stay in bootmode and wait until someone sends us
        // a binary via u(s)art
        let rec = ImageReceiver::new(&mut flasher, &mut uart);
        rec.execute(update_info_address);
        // after we received the binary we just reboot. We'll endup in this function again
        // with a hopefully wellformed update_info which can be installed and booted.        
    }
}