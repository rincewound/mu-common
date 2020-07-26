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

pub struct ImageReceiver<'a, T: Flasher, U: Read<u8> + Write<u8> >
{
    flasher: &'a mut T,
    uart: &'a mut U,
    done: bool,
    current_address: usize,
    image_info: Option<super::update_info>
}

impl <'a, T: Flasher, U: Read<u8> + Write<u8>> ImageReceiver<'a, T,U>
{
    pub fn new(flasher: &'a mut T, uart: &'a mut U) -> Self
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

        // Note: At this point we could check if we received enough bytes (as indicated by
        // the infostruct), however: If we did not receive enough bytes the CRC check should
        // fail, thus not writing the update struct to flash.

        // check the received image's CRC against the update_info_struct
        if let Some(update_struct) = self.image_info
        {
            if crc::check_crc(update_struct.update_start, update_struct.update_len, update_struct.checksum, self.flasher)
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
        let version = payload[5];        
        let start_area = usize_from_packet(&payload, 6);
        let upd_len = usize_from_packet(&payload, 10);
        let target_adr = usize_from_packet(&payload, 14);
        let checksum = usize_from_packet(&payload, 18);

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

    /// This function is guaranteed to return 
    /// a received byte. However, this means
    /// it will block forever, if nothing
    /// arrives. 
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
                bytes_to_receive = 22;
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
mod test
{
    use crate::testhelpers::*;
    use crate::Flasher;

    #[test]
    pub fn can_exit_updater_when_sending_end_packet()
    {
        let mut uart = FakeUart::new();
        make_packet(&mut uart, &[super::STX, super::END, super::ETX, 0x05]);

        let mut flasher = FakeFlasher::new();

        let r = super::ImageReceiver::new(&mut flasher, & mut uart);
        r.execute(0x1000); 
        assert!(uart.out_buf[0] == super::ACK)       
    }

    #[test]
    pub fn will_respond_with_nak_on_bad_bcc()
    {
        let mut uart = FakeUart::new();
        let packet = [super::STX, super::END, super::ETX, 0x77];
        copy_to_uart(&mut uart, &packet);

        // We need to send the second packet as well to actually terminate the
        // execute loop
        make_packet(&mut uart, &[super::STX, super::END, super::ETX, 0x05]);

        let mut flasher = FakeFlasher::new();

        let r = super::ImageReceiver::new(&mut flasher, & mut uart);
        r.execute(0x1000);         
        assert!(uart.out_buf[0] == super::NAK)        
    }

    #[test]
    pub fn init_will_start_the_update()
    {
        // If we send a valid update start and then a datapacket, 
        // the packet should end up at the specified locatsion
        let mut uart = FakeUart::new();
        let packet = [super::STX, 
                                super::INIT,             // Packet Type
                                b'M', b'U', b'U', b'P', b'D', // Magic
                                0x01,                    // Struct Version                                
                                0x00, 0x00, 0x20, 0x00,  // write to 0x2000
                                0x00, 0x00, 0x00, 0x80,  // 128 byte update len
                                0x00, 0x00, 0x40, 0x00,  // Installation area is 0x4000
                                0xAB, 0xCD, 0xEF, 0xAA,  // CRC
                                super::ETX];
        make_packet(&mut uart, &packet);


        let packet2  = [super::STX, super::DATA, 
                                 1,2,3,4,5,6,7,8,
                                 1,2,3,4,5,6,7,16,
                                 1,2,3,4,5,6,7,24,
                                 1,2,3,4,5,6,7,32,
                                 1,2,3,4,5,6,7,40,
                                 1,2,3,4,5,6,7,48,
                                 1,2,3,4,5,6,7,56,
                                 1,2,3,4,5,6,7,64,
                                 1,2,3,4,5,6,7,72,
                                 1,2,3,4,5,6,7,80,
                                 1,2,3,4,5,6,7,88,
                                 1,2,3,4,5,6,7,96,
                                 1,2,3,4,5,6,7,104,
                                 1,2,3,4,5,6,7,112,
                                 1,2,3,4,5,6,7,120,
                                 1,2,3,4,5,6,7,128,
                                 super::ETX];
        make_packet(&mut uart, &packet2);        
        make_packet(&mut uart, &[super::STX, super::END, super::ETX]);

        let mut flasher = FakeFlasher::new();

        let r = super::ImageReceiver::new(&mut flasher, & mut uart);
        r.execute(0x1000); 

        // read back data:
        for i in 1..8
        {
            let mut buf: [u8; 1] = [0];
            let _ = flasher.read((0x2000 + i * 8) - 1, &mut buf[..]);
            assert!(buf[0] == (i * 8) as u8);
        }

    }
}