# muload
A bootloader framework for microcontrollers

## Features
- [x] Red/Green Deployments: Download a binary, check for consistency before actually installing to flash. 
- [x] Low RAM usage
- [x] Download via UART if required
- [x] Mostly safe code (this is a bootloader - we need a bit of unsafe stuff.)
- [ ] LZMA Decoding
- [ ] Salsa20 encrypted binaries

## Supports
Ports are available for
- [ ] STM32L4F401/Nucleo (Cortex M4)
- [ ] nRF52810 (nordic DK/ Cortex M4)

## Concepts

### Endianness
All structs assume little endian byteorder. At we don't support big endian machines.

### The bin_info Struct
The bin_info struct contains all relevant information that is needed to launch the application, most notably:
* The start address of the app
* The checksum of the app.

Structure
```
struct bin_info
{
    magic: [u8;5],
    struct_ver: u8,
    app_start: u32,
    app_len: u32,
    app_checksum: u32,
    info_checksum: u32
}
```

Note: the "magic" field will always contain the bytes b"MUBIN". The bin_info struct is located at a known address with the name:
```
 extern "C" { __bin_info_adress: u32 }
```

The struct_ver field shall always have the value 0x01 (with the version being 1 at the moment.


### The update_info Struct
Structure
```
struct update_info
{
    magic: [u8;5],
    struct_ver: u8,
    update_start: u32,
    update_len: u32,
    target_adress: u32,
    update_encoding: UpdateEncoding,
    update_checksum: u32,
    info_checksum
}
```
Note: the "magic" field will always contain the bytes b"MUUPD". The update_info struct is located at a known address with the name:
```
 extern "C" { __update_info_adress: u32 }
```

The struct_ver field shall always have the value 0x01 (with the version being 1 at the moment.

The valid values for UpdateEncoding are
```
enum UpdateEncoding
{
    Raw = 0,
    LZMA = 1
}
```

## The binary format
muload assumes, that a given binary is immediately executable, after it was flashed to the target memory area


## Assumptions
muload requires an implementation of the "flasher" trait to be passed to the update. The flasher will have to take into account the specifics of the target's flash (e.g. page size, flashing algorithm). Muload makes the following assumptions with respect to the flash characteristics:
* It is allowed to read from any valid address
* It is allowed to sequentially write to flash in arbitrary chunksizes.
* The UpdateInfoStruct and the BinInfoStruct can be deleted independently of each other.

The last assumption might require the flasher to buffer data at times, however it is necessary 

## Usage
### Flashing using a UART
muload will attempt to boot using a given bin_info location. If it does not find a valid application there (i.e. either bin_info is invalid or does not point to a valid app (as defined by having a valid checksum)) it will stay in bootmode and try to receive an update binary via UART.

Apart from this the loader will emit a single "B" byte on the UART upon boot. If it receives a download request within 100 ms it will not attempt to boot the resident image and instead initiate an image download.

### The muload UART Protocol

A muload packet has the following layout:


| STX |Type| Payload | ETX | BCC |
|-----|----|---------|-----|------

Where 
* STX = 0x02
* ETX = 0x03
* Type denotes the packet type.
* (optional) Payload is a zeropadded 128 byte chunk of data
* BCC is the XOR checksum over the rest of the packet including the framing.

The packettype can be either:
* Init Download (0x16/SYN). (Re-) Starts the download. The payload of this packet contains the update_info_struct for this update
* Data (0x01/SOH): Contains a datapacket (i.e. with payload!)
* End Download (0x04/EOT): Notifies the bootloader that the download is finished.


The loader will respond to each packet either with ACK (0x06), denoting a completely received packet, or with NAK (0x15), denoting either a bad checksum or an unsupported packettype. Note that, when the loader received a DATA packet successfully it will immediately write the data to flash (i.e. before sending the ACK), which might take some time, depending on the type of flash used by the MCU and on wether or not a new page was started. If the loader answers with NAK the host can choose to resend the packet or to abort by sending an End Download command.

## Customizing for a given MCU