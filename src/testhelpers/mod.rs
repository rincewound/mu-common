use embedded_hal::serial::{Read, Write};
use nb;
use crate::Flasher;

pub enum SomeEnum { }

pub struct FakeUart
{
    pub memory: [u8; 512],
    pub out_buf: [u8; 512],
    pub read_index: usize,
    pub write_index: usize,
    pub mem_use: usize
}

impl FakeUart
{
    pub fn new() -> Self
    {
        Self
        {
            memory: [0; 512],
            out_buf: [0; 512],
            read_index: 0,
            write_index: 0,
            mem_use: 0
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

pub struct FakeFlasher
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

    fn read(&self, source_address: usize, destination: &mut[u8]) -> Result<usize, crate::ReadError> 
    {

        for index in 0..destination.len()
        {
            destination[index] = self.memory[source_address + index];
        }
        Ok(destination.len())
    }

    fn flush(&mut self) {
        self.flush_called = true;           
    }
}

pub fn copy_to_uart( uart: &mut FakeUart, data: &[u8])
{
    for byte in data.iter()
    {
        uart.memory[uart.mem_use] = *byte;
        uart.mem_use += 1
    }

}

pub fn copy_to_flasher( flasher: &mut FakeFlasher, data: &[u8])
{
    for (index, byte) in data.iter().enumerate()
    {
        flasher.memory[index] = *byte;
    }
}

pub fn make_packet(uart: &mut FakeUart, data: &[u8])
{
    let start_index = uart.mem_use;
    copy_to_uart(uart, data);

    // calc checksum:
    let mut bcc: u8 = 0;
    for i in start_index.. start_index + data.len()
    {
        bcc ^= uart.memory[i];
    }
    uart.memory[uart.mem_use ] = bcc;
    uart.mem_use += 1;

}