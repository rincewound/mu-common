
use super::Flasher;

pub fn check_crc<T>(start_adr: usize, len: usize, checksum: usize, flasher: &T) -> bool
    where T: Flasher
{
    let mut crc = 0xFFFFFFFF as u32;
    let mut bytes_left = len;
    let mut index = 0;
    while bytes_left > 0
    {
        let mut buf: [u8; 64] = [0;64];
        if let Ok(num_bytes_read) = flasher.read(start_adr + index, &mut buf)
        {
            let num_bytes_to_process: usize;

            if num_bytes_read > len
            {
                num_bytes_to_process = len;
            }
            else
            {
                num_bytes_to_process = num_bytes_read;
            }

            for b in 0..num_bytes_to_process
            {
                let mut val = ((crc ^ (buf[b] as u32)) & 0xFF) as u32;
                for _ in 0..8
                {
                    if val & 1 != 0
                    {
                        val = (val >> 1) ^ (0xEDB88320 as u32);
                    }
                    else
                    {
                        val = val >> 1;
                    }
                }
                crc = val ^ crc >> 8;
            }

            bytes_left -= num_bytes_to_process;
            index += num_bytes_read;
        }
        else
        {
            // Read failure, we assume a bad crc in this case.
            return false;
        }
    }

    return checksum == crc as usize ^ 0xFFFFFFFF as usize;
}


#[cfg(test)]
mod test
{
    use crate::testhelpers::*;
    use super::check_crc;

    #[test]
    fn can_calc_crc()
    {
        let mut fl = FakeFlasher::new();
        copy_to_flasher(&mut fl, &[0xAA,0xBB,0xCC,0xDD,0xEE,0xFF,0x11,0x22]);        
        assert!(true == check_crc(0, 8, 0x65133A42, &mut fl))
    }
}