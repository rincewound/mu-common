use super::Flasher;
use super::crc;
use embedded_hal::serial::{Read, Write};

const STX: u8 = 0x02;
const ETX: u8 = 0x03;

const INIT: u8 = 0x16;
const DATA: u8 = 0x01;
const END: u8 = 0x04;
const NAK: u8 = 0x15;
const ACK: u8 = 0x06;

struct Packet
{
    packettype: u8,
    data: Option<[u8;128]>
}

fn usize_from_packet(packet_data: &[u8], index: usize) -> usize
{
    let result = ((packet_data[index] as u32) << 24 |
                 (packet_data[index + 1]as u32) << 16 |
                 (packet_data[index + 2]as u32) << 8 |
                 (packet_data[index + 3]as u32)) as usize;
    result
}

pub struct ImageReceiver<T: Flasher, U: Read<u8> + Write<u8> >
{
    flasher: T,
    uart: U,
    done: bool,
    current_address: usize,
    image_info: Option<super::update_info>
}

impl <T: Flasher,U: Read<u8> + Write<u8>> ImageReceiver<T,U>
{
    pub fn new(flasher: T, uart: U) -> Self
        where T: Flasher, U: Read<u8> + Write<u8> 
    {
        Self
        {
            flasher, 
            uart, 
            done:false, 
            current_address: 0,
            image_info: None
        }
    }

    pub fn execute(mut self, update_info_address: usize)
    {
        while !self.done
        {
            if let Some(packet) = self.receive_packet()
            {
                if !self.dispatch_packet(packet)
                {
                    let _ = self.uart.write(NAK);
                }
                else
                {
                    let _ = self.uart.write(ACK);
                }
            }
        }

        // check the received image's CRC against the update_info_struct
        if let Some(update_struct) = self.image_info
        {
            if crc::check_crc(update_struct.update_start, update_struct.update_len, update_struct.checksum, &self.flasher)
            {
                // Write the update struct as well
                let num_bytes = core::mem::size_of::<super::update_info>();
                let data_slice = unsafe {core::slice::from_raw_parts((&update_struct as *const super::update_info) as *const u8, num_bytes)};
                let _= self.flasher.write(update_info_address, data_slice);
            }
        }
    }

    fn dispatch_packet(&mut self, packet: Packet) -> bool
    {
        match packet.packettype
        {
            INIT => return self.init_update(packet),
            DATA => return self.flash_data(packet).is_ok(),
            END => return self.end_update(),
            _ => return false
        }
    }

    fn init_update(&mut self, packet: Packet) -> bool
    {
        let payload = packet.data.unwrap();
        let version = payload[6];        
        let start_area = usize_from_packet(&payload, 7);
        let upd_len = usize_from_packet(&payload, 11);
        let target_adr = usize_from_packet(&payload, 15);
        let checksum = usize_from_packet(&payload, 19);

        // ToDo: Check if we actually received the correct magic value.
        self.image_info = Some(super::update_info {
            magic: ['M' as u8, 'U' as u8, 'U' as u8, 'P' as u8, 'D' as u8],
            struct_ver: version,
            update_start: start_area,
            update_len: upd_len,
            target_adress: target_adr,
            checksum: checksum,
        });

        self.current_address = start_area;

        true
    }

    fn flash_data(&mut self, packet: Packet) -> Result<(), super::WriteError>
    {
        let result = self.flasher.write(self.current_address, &packet.data.unwrap());
        if result.is_ok()
        {
            self.current_address += 128;
        }
        result
    }

    fn end_update(&mut self) -> bool
    {
        self.flasher.flush();
        self.done = true;
        return true;
    }

    fn get_byte(&mut self) -> u8
    {
        loop 
        {
            let read_result = self.uart.read();
            if let Ok(byte ) = read_result
            {
                return byte;
            }
        }
    }

    fn receive_packet(&mut self) -> Option<Packet>
    {
        loop 
        {
            if self.get_byte() == STX
            {                
                break
            }
        }

        let packet_type = self.get_byte();

        let mut bcc = 0x00 ^ STX ^ packet_type;
        let mut received_data: Option<[u8;128]> = None;

        if packet_type == DATA || packet_type == INIT
        {
            let mut bytes_to_receive = 128;
            if packet_type == INIT
            {
                bytes_to_receive = 24;
            }

            let mut payload: [u8; 128] = [0;128];
            for i in 0..bytes_to_receive
            {
                payload[i] = self.get_byte();
                bcc ^= payload[i];
            }
            received_data = Some(payload);
        }

        let etx = self.get_byte();

        // we should have received an etx now, if not, something has gone
        // wrong.

        if etx != ETX
        {
            return None;
        }

        bcc ^= etx;

        let received_bcc = self.get_byte();

        if bcc != received_bcc
        {
            // answer NAK, return None
            let _ = self.uart.write(NAK);
            return None;
        }
        
        return Some(Packet{packettype: packet_type, data: received_data});
    }
}

#[cfg(test)]
mod target_adress
{
    use embedded_hal::serial::{Read, Write};
    use nb;
    use super::Flasher;

    pub enum SomeEnum {
        Fail
    }

    struct FakeUart
    {
        pub memory: [u8; 256],
        pub out_buf: [u8; 256],
        pub read_index: usize,
        pub write_index: usize
    }

    impl FakeUart
    {
        pub fn new() -> Self
        {
            Self
            {
                memory: [0; 256],
                out_buf: [0; 256],
                read_index: 0,
                write_index: 0
            }
        }
    }

    impl Read::<u8> for FakeUart
    {
        type Error = SomeEnum;
        fn read(&mut self) -> nb::Result<u8, Self::Error> {
            if self.read_index > 256
            {
                return Err(nb::Error::WouldBlock)
            }
            let result = self.memory[self.read_index];
            self.read_index += 1;
            Ok(result)
        }
    }

    impl Write::<u8> for FakeUart
    {
        type Error = SomeEnum;
        fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> 
        {
            self.out_buf[self.write_index] = word;
            self.write_index += 1;
            Ok(())
        }
        fn flush(&mut self) -> nb::Result<(), Self::Error> 
        {
            todo!()
        }
        
    }

    struct FakeFlasher
    {
        memory: [u8; 0x8000],
        flush_called: bool
    }

    impl FakeFlasher
    {
        pub fn new() -> Self
        {
            Self
            {
                memory: [0x00; 0x8000],
                flush_called: false
            }
        }
    }

    impl Flasher for FakeFlasher
    {
        fn write(&mut self, destination: usize, data: &[u8]) -> Result<(), crate::WriteError> {
            for (index, byte) in data.iter().enumerate()
            {
                self.memory[destination + index as usize] = *byte;
            }
            return Ok(());
        }

        fn read(&self, source_address: usize, destination: &mut[u8]) -> Result<usize, crate::ReadError> {
        todo!()
        }
        fn flush(&mut self) {
            self.flush_called = true;           
        }
    }

    fn copy_to_uart( uart: &mut FakeUart, data: &[u8])
    {
        for (index, byte) in data.iter().enumerate()
        {
            uart.memory[index] = *byte;
        }
    }

    fn copy_to_flasher( uart: &mut FakeFlasher, data: &[u8])
    {
        for (index, byte) in data.iter().enumerate()
        {
            uart.memory[index] = *byte;
        }
    }


    #[test]
    pub fn can_exit_updater_when_sending_end_packet()
    {
        let mut uart = FakeUart::new();
        let packet = [super::STX, super::END, super::ETX, 0x05];
        copy_to_uart(&mut uart, &packet);
        let r = super::ImageReceiver::new(FakeFlasher::new(), uart);
        r.execute(0x1000);        
    }
}