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

    pub fn execute(mut self) -> !
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
            }
        }

        loop{}
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
            if let Ok(byte ) = self.uart.read()
            {
                return byte;
            }
        }
    }

    fn receive_packet(&mut self) -> Option<Packet>
    {
        loop 
        {
            if self.get_byte() != STX
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