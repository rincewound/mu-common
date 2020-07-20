
use super::Flasher;

pub fn check_crc<T>(start_adr: usize, len: usize, checksum: u32, flasher: &T) -> bool
    where T: Flasher
{
    false
}