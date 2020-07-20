
use super::Flasher;

pub fn check_crc<T>(start_adr: usize, len: usize, checksum: u32, flasher: &T) -> bool
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
            for b in 0..num_bytes_read
            {
                let mut val = crc ^ (buf[b] & 0xFF) as u32;
                for _ in 0..8
                {
                    if val & 1 == 1
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

            bytes_left -= num_bytes_read;
            index += num_bytes_read;
        }
        else
        {
            return false;
        }
    }

    return checksum == crc as u32 ^ 0xFFFFFFFF as u32;
}

// uint32_t CRC32_function(uint8_t *buf, uint32_t len){

//     uint32_t val, crc;
//     uint8_t i;

//     crc = 0xFFFFFFFF;
//     while(len--){
//         val=(crc^*buf++)&0xFF;
//         for(i=0; i<8; i++){
//             val = val & 1 ? (val>>1)^0xEDB88320 : val>>1;
//         }
//         crc = val^crc>>8;
//     }
//     return crc^0xFFFFFFFF;
// }