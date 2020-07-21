use super::{bin_info, Flasher, crc};


pub fn check_binary<T>(data: &bin_info, flasher: &T) -> bool
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

pub fn launch_binary(data: bin_info) ->!
{
    unsafe 
    {
        let app_entry = core::mem::transmute::<usize, fn() -> !>(data.app_start);
        app_entry();
    }
}