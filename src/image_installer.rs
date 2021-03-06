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

    // if data.update_encoding != UpdateEncoding::Raw
    // {
    //     return false;
    // }

    return crc::check_crc(data.update_start, data.update_len, data.checksum, flasher);
}

pub fn install_binary<T>(data: &update_info, flasher: &mut T) -> bool
where T: Flasher
{
    const BUF_SIZE: usize = 64;
    let mut buff: [u8; BUF_SIZE] = [0; BUF_SIZE];
    let mut bytes_left = data.update_len as usize;
    let mut bytes_written: usize = 0;
    while bytes_left > 0
    {
        if let Ok(result) = flasher.read(data.update_start + bytes_written, &mut buff)
        {
            let bytes_to_copy = if result > bytes_left { bytes_left } else { result };
            let dst_slice = &buff[0..bytes_to_copy];

            if let Ok(()) = flasher.write(data.target_adress + bytes_written, dst_slice)
            {
                if bytes_left > result
                {
                    bytes_left = bytes_left - result;
                }
                else
                {
                    bytes_left = 0;
                }
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

    // ToDo: Write Bin_Info with data from update_info 

    return crc::check_crc(data.target_adress, data.update_len, data.checksum, flasher);
}

#[cfg(test)]
mod test
{
    use crate::{update_info, testhelpers::FakeFlasher, testhelpers::copy_to_flasher};
    use super::{check_update, install_binary};


    #[test]
    pub fn check_update_will_yield_false_if_magic_word_is_missing()
    {
        let fl = FakeFlasher::new();
        let update_info = update_info {
            magic: [b'M', b'M', b'M', b'M', b'M'],
            struct_ver: 1,
            update_len: 100,
            update_start: 0x1000,
            target_adress: 0x4000,
            checksum: 0x9988C6CA
        };

        assert!(false == check_update(&update_info, &fl));
    }

    #[test]
    pub fn check_update_will_yield_false_if_struct_ver_is_bad()
    {
        let fl = FakeFlasher::new();
        let update_info = update_info {
            magic: [b'M', b'U', b'U', b'P', b'D'],
            struct_ver: 2,
            update_len: 100,
            update_start: 0x1000,
            target_adress: 0x4000,
            checksum: 0x9988C6CA
        };

        assert!(false == check_update(&update_info, &fl));       
    }

    #[test]
    pub fn check_update_will_yield_false_if_checksum_is_bad()
    {
        let fl = FakeFlasher::new();
        let update_info = update_info {
            magic: [b'M', b'U', b'U', b'P', b'D'],
            struct_ver: 1,
            update_len: 100,
            update_start: 0x1000,
            target_adress: 0x4000,
            checksum: 0xC0FFEE
        };

        assert!(false == check_update(&update_info, &fl));       
    }

    #[test]
    pub fn check_update_will_yield_true_if_no_error()
    {
        let fl = FakeFlasher::new();
        let update_info = update_info {
            magic: [b'M', b'U', b'U', b'P', b'D'],
            struct_ver: 1,
            update_len: 100,
            update_start: 0x1000,
            target_adress: 0x4000,
            checksum: 0x9988C6CA
        };

        assert!(true == check_update(&update_info, &fl));       
    }

    #[test]
    pub fn install_binary_will_copy_bytes()
    {
        let mut fl = FakeFlasher::new();
        let binary: [u8;5] = [0xAA, 0xBB, 0xCC, 0xDD, 0x11];
        copy_to_flasher(&mut fl, 0x1000, &binary);

        let update_info = update_info {
            magic: [b'M', b'U', b'U', b'P', b'D'],
            struct_ver: 1,
            update_len: 5,
            update_start: 0x1000,
            target_adress: 0x4000,
            checksum: 0x9988C6CA
        };       


        install_binary(&update_info, &mut fl);

        for i in 0..5
        {
            assert!(fl.memory[i + 0x4000] == binary[i]);
        }

        assert!(fl.flush_called)
    }


}