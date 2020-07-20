use super::{update_info, Flasher, UpdateEncoding, crc};


pub fn check_update<T>(data: &update_info, flasher: &T) -> bool
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

pub fn install_binary<T>(data: &update_info, flasher: &mut T) -> bool
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
