use mini_hypervisor::error::{Error as HypervisorError, PortIoError};
use mini_hypervisor::execution::run_vcpu_until_stopped;
use mini_hypervisor::kvm::KvmBackend;
use mini_hypervisor::memory::{GuestMemory, GuestPhysAddr};
use mini_hypervisor::portio::PortIoBus;
use mini_hypervisor::vcpu::VcpuId;
use std::error::Error;
use std::fs;
use std::io;
use std::path::Path;

const GUEST_RAM_SIZE: u64 = 64 * 1024 * 1024;
const MULTIBOOT_INFO_ADDR: u64 = 0x5000;
const KERNEL_ENTRY_ADDR: u64 = 0x6000;
const BRIDGE_ADDR: u64 = 0x7000;
const MULTIBOOT_BOOTLOADER_MAGIC: u32 = 0x2bad_b002;
const MULTIBOOT_INFO_MEMORY: u32 = 1 << 0;
const EXPECTED_BANNER: &[u8] = b"Booting Advanced OS...\n";
const EXPECTED_BOUNDARY_PORT: u16 = 0x20;
const KVM_IO_OUT: u8 = 1;
const EXIT_BUDGET: u32 = 128;
const PT_LOAD: u32 = 1;

#[derive(Debug, Clone, Copy)]
struct Elf32LoadSegment {
    file_offset: usize,
    load_address: u64,
    file_size: usize,
    memory_size: usize,
}

#[derive(Debug)]
struct Elf32Image {
    entry: u32,
    segments: Vec<Elf32LoadSegment>,
}

fn invalid(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(io::ErrorKind::InvalidData, message.into()))
}

fn checked_range(
    length: usize,
    offset: usize,
    size: usize,
) -> Result<std::ops::Range<usize>, Box<dyn Error>> {
    let end = offset
        .checked_add(size)
        .ok_or_else(|| invalid("ELF range overflow"))?;
    if end > length {
        return Err(invalid(format!(
            "ELF range {offset:#x}..{end:#x} exceeds file length {length:#x}"
        )));
    }
    Ok(offset..end)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, Box<dyn Error>> {
    let range = checked_range(bytes.len(), offset, 2)?;
    Ok(u16::from_le_bytes([
        bytes[range.start],
        bytes[range.start + 1],
    ]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Box<dyn Error>> {
    let range = checked_range(bytes.len(), offset, 4)?;
    Ok(u32::from_le_bytes([
        bytes[range.start],
        bytes[range.start + 1],
        bytes[range.start + 2],
        bytes[range.start + 3],
    ]))
}

fn parse_elf32(bytes: &[u8]) -> Result<Elf32Image, Box<dyn Error>> {
    if bytes.len() < 52 {
        return Err(invalid("MinIOS kernel is shorter than an ELF32 header"));
    }
    if &bytes[..4] != b"\x7fELF" {
        return Err(invalid("MinIOS kernel is not ELF"));
    }
    if bytes[4] != 1 || bytes[5] != 1 || bytes[6] != 1 {
        return Err(invalid(
            "MinIOS kernel must be ELF32 little-endian version 1",
        ));
    }
    if read_u16(bytes, 16)? != 2 {
        return Err(invalid("MinIOS kernel must be an ELF executable"));
    }
    if read_u16(bytes, 18)? != 3 {
        return Err(invalid("MinIOS kernel must target 32-bit x86"));
    }

    let entry = read_u32(bytes, 24)?;
    let phoff = usize::try_from(read_u32(bytes, 28)?)?;
    let phentsize = usize::from(read_u16(bytes, 42)?);
    let phnum = usize::from(read_u16(bytes, 44)?);
    if phentsize < 32 || phnum == 0 {
        return Err(invalid("MinIOS kernel has no usable ELF32 program headers"));
    }

    let table_size = phentsize
        .checked_mul(phnum)
        .ok_or_else(|| invalid("ELF32 program-header table size overflow"))?;
    checked_range(bytes.len(), phoff, table_size)?;

    let mut segments = Vec::new();
    for index in 0..phnum {
        let header = phoff
            .checked_add(
                index
                    .checked_mul(phentsize)
                    .ok_or_else(|| invalid("ELF32 program-header offset overflow"))?,
            )
            .ok_or_else(|| invalid("ELF32 program-header offset overflow"))?;
        if read_u32(bytes, header)? != PT_LOAD {
            continue;
        }

        let file_offset = usize::try_from(read_u32(bytes, header + 4)?)?;
        let virtual_address = read_u32(bytes, header + 8)?;
        let physical_address = read_u32(bytes, header + 12)?;
        let file_size = usize::try_from(read_u32(bytes, header + 16)?)?;
        let memory_size = usize::try_from(read_u32(bytes, header + 20)?)?;
        if memory_size < file_size {
            return Err(invalid(format!(
                "PT_LOAD[{index}] has p_memsz {memory_size:#x} smaller than p_filesz {file_size:#x}"
            )));
        }
        checked_range(bytes.len(), file_offset, file_size)?;
        let load_address = u64::from(if physical_address == 0 {
            virtual_address
        } else {
            physical_address
        });
        let end = load_address
            .checked_add(u64::try_from(memory_size)?)
            .ok_or_else(|| invalid("PT_LOAD guest range overflow"))?;
        if end > GUEST_RAM_SIZE {
            return Err(invalid(format!(
                "PT_LOAD[{index}] ends at {end:#x}, outside {GUEST_RAM_SIZE:#x}-byte guest RAM"
            )));
        }

        segments.push(Elf32LoadSegment {
            file_offset,
            load_address,
            file_size,
            memory_size,
        });
    }

    if segments.is_empty() {
        return Err(invalid("MinIOS kernel contains no PT_LOAD segments"));
    }
    if u64::from(entry) >= GUEST_RAM_SIZE {
        return Err(invalid(format!(
            "MinIOS entry {entry:#x} lies outside guest RAM"
        )));
    }

    Ok(Elf32Image { entry, segments })
}

fn load_elf32(
    memory: &mut GuestMemory,
    bytes: &[u8],
    image: &Elf32Image,
) -> Result<(), Box<dyn Error>> {
    for segment in &image.segments {
        let file = checked_range(bytes.len(), segment.file_offset, segment.file_size)?;
        memory.write(GuestPhysAddr::new(segment.load_address), &bytes[file])?;

        let zero_count = segment.memory_size - segment.file_size;
        if zero_count != 0 {
            let zero_address = segment
                .load_address
                .checked_add(u64::try_from(segment.file_size)?)
                .ok_or_else(|| invalid("PT_LOAD BSS address overflow"))?;
            memory.write(GuestPhysAddr::new(zero_address), &vec![0_u8; zero_count])?;
        }
    }
    Ok(())
}

fn install_multiboot_info(memory: &mut GuestMemory) -> Result<(), Box<dyn Error>> {
    // Multiboot v1 starts with flags, mem_lower, mem_upper. We deliberately set only
    // MULTIBOOT_INFO_MEMORY; every later field remains zero and must therefore be ignored.
    let mut info = [0_u8; 116];
    info[0..4].copy_from_slice(&MULTIBOOT_INFO_MEMORY.to_le_bytes());
    info[4..8].copy_from_slice(&640_u32.to_le_bytes());
    let upper_kib = u32::try_from(GUEST_RAM_SIZE / 1024 - 1024)?;
    info[8..12].copy_from_slice(&upper_kib.to_le_bytes());
    memory.write(GuestPhysAddr::new(MULTIBOOT_INFO_ADDR), &info)?;
    Ok(())
}

fn run(kernel_path: &Path, bridge_path: &Path) -> Result<(), Box<dyn Error>> {
    let kernel = fs::read(kernel_path)?;
    let bridge = fs::read(bridge_path)?;
    if bridge.is_empty() {
        return Err(invalid("Multiboot bridge binary is empty"));
    }
    let bridge_end = BRIDGE_ADDR
        .checked_add(u64::try_from(bridge.len())?)
        .ok_or_else(|| invalid("Multiboot bridge range overflow"))?;
    if bridge_end >= 0x1_0000 {
        return Err(invalid(
            "Multiboot bridge no longer fits the real-mode low-memory window",
        ));
    }

    let image = parse_elf32(&kernel)?;
    let backend = KvmBackend::open()?;
    let mut vm = backend.create_vm()?;
    let mut memory = GuestMemory::new(GuestPhysAddr::new(0), GUEST_RAM_SIZE)?;

    load_elf32(&mut memory, &kernel, &image)?;
    install_multiboot_info(&mut memory)?;
    memory.write(
        GuestPhysAddr::new(KERNEL_ENTRY_ADDR),
        &image.entry.to_le_bytes(),
    )?;
    memory.write(GuestPhysAddr::new(BRIDGE_ADDR), &bridge)?;
    vm.register_guest_memory(memory)?;

    let mut vcpu = vm.create_vcpu(VcpuId::BOOT)?;
    vcpu.initialize_real_mode(GuestPhysAddr::new(BRIDGE_ADDR))?;
    let mut port_io = PortIoBus::with_debug_port();
    let execution = run_vcpu_until_stopped(&mut vcpu, &mut port_io, EXIT_BUDGET);
    let debug_output = port_io.debug_output().unwrap_or(&[]);

    if debug_output != EXPECTED_BANNER {
        return Err(invalid(format!(
            "MinIOS debug output mismatch: expected {:?}, observed {:?}",
            EXPECTED_BANNER, debug_output
        )));
    }

    match execution {
        Err(HypervisorError::PortIo(PortIoError::UnhandledPort {
            port,
            direction,
            size,
            count,
        })) if port == EXPECTED_BOUNDARY_PORT && direction == KVM_IO_OUT && size == 1 && count == 1 => {
            println!("MinIOS ELF32 entry: {:#x}", image.entry);
            println!("MinIOS PT_LOAD segments: {}", image.segments.len());
            println!("MinIOS Multiboot magic: {MULTIBOOT_BOOTLOADER_MAGIC:#x}");
            println!("MinIOS debug proof: {}", String::from_utf8_lossy(debug_output).escape_debug());
            println!("MinIOS boundary: OUT port={port:#x} size={size} count={count}");
            println!("hypervisor -> MinIOS early-boot proof: VERIFIED");
            Ok(())
        }
        Err(error) => Err(invalid(format!(
            "MinIOS reached the boot banner but stopped at an unexpected hypervisor boundary: {error:?}"
        ))),
        Ok(result) => Err(invalid(format!(
            "MinIOS unexpectedly stopped without the required PIC I/O boundary: {:?}",
            result.report()
        ))),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os();
    let program = args.next().unwrap_or_default();
    let kernel = args.next().ok_or_else(|| {
        invalid(format!(
            "usage: {} <minios-kernel.bin> <multiboot-bridge.bin>",
            Path::new(&program).display()
        ))
    })?;
    let bridge = args
        .next()
        .ok_or_else(|| invalid("missing Multiboot bridge path"))?;
    if args.next().is_some() {
        return Err(invalid("unexpected extra arguments"));
    }
    run(Path::new(&kernel), Path::new(&bridge))
}
