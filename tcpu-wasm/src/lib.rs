#![cfg(target_arch = "wasm32")]
#![cfg_attr(not(test), no_std)]
#![allow(unused)]

mod alloc;

use alloc::Box;
use core::convert::TryInto;
use core::fmt;
use core::mem::MaybeUninit;
use tcpu::{Disk, DiskId, Emulator, Storage, DISK_SIZE, MEMORY_SIZE};

impl Default for Box<[u8; tcpu::MEMORY_SIZE]> {
    fn default() -> Self {
        unsafe { Self::new_zeroed() }
    }
}

impl Storage<[u8; MEMORY_SIZE]> for Box<[u8; MEMORY_SIZE]> {
    fn as_ref(&self) -> &[u8; MEMORY_SIZE] { self }
    fn as_mut(&mut self) -> &mut [u8; MEMORY_SIZE] { self }
}

impl Default for Box<[u8; tcpu::DISK_SIZE]> {
    fn default() -> Self {
        unsafe { Self::new_zeroed() }
    }
}

impl Storage<[u8; DISK_SIZE]> for Box<[u8; DISK_SIZE]> {
    fn as_ref(&self) -> &[u8; DISK_SIZE] { self }
    fn as_mut(&mut self) -> &mut [u8; DISK_SIZE] { self }
}

fn log(message: &str) {
    extern { pub fn log_message(ptr: *const u8, len: usize); }
    let bytes = message.as_bytes();
    unsafe { log_message(bytes.as_ptr(), bytes.len()) };
}

fn abort(message: &str) -> ! {
    log(message);
    unsafe { core::arch::wasm32::unreachable() }
}

struct RuntimeData {
    emulator: Emulator<Box<[u8; MEMORY_SIZE]>, Box<[u8; DISK_SIZE]>>,
}

static mut DATA: MaybeUninit<RuntimeData> = MaybeUninit::uninit();

#[no_mangle]
pub extern fn initialize() {
    unsafe {
        DATA = MaybeUninit::new(RuntimeData {
            emulator: Emulator::new(),
        });
    }
}

unsafe fn get_runtime_data() -> &'static mut RuntimeData {
    &mut *DATA.as_mut_ptr()
}

macro_rules! export {
    ($(fn $name:ident($($data:ident: &mut RuntimeData $(, $($args:tt)*)?)?) $(-> $ret:ty)? $body:block)*) => {
        $(const _: () = {
            #[no_mangle]
            pub extern fn $name($($($($args)*)?)?) $(-> $ret)? {
                $(let $data = unsafe { get_runtime_data() };)?
                $body
            }
        };)*
    };
}

struct DiskStats {
    present: bool,
    modified: bool,
    idle_time: u32,
}

impl DiskStats {
    fn missing() -> Self {
        DiskStats {
            present: false,
            modified: false,
            idle_time: 0,
        }
    }
    
    fn as_bits(&self) -> u32 {
        (core::cmp::min(self.idle_time, u32::max_value() >> 2) << 2) |
        (u32::from(self.modified) << 1) |
        u32::from(self.present)
    }
}

export! {
    fn run(data: &mut RuntimeData, cycles: u32) {
        data.emulator.run(cycles.into());
    }

    fn reset(data: &mut RuntimeData) {
        data.emulator.reset();
    }

    fn screen_buffer(data: &mut RuntimeData) -> *const u8 {
        data.emulator.screen().as_ptr() as *const u8
    }

    fn screen_width() -> u32 {
        tcpu::SCREEN_WIDTH as u32
    }
    
    fn screen_height() -> u32 {
        tcpu::SCREEN_HEIGHT as u32
    }

    fn is_running(data: &mut RuntimeData) -> bool {
        data.emulator.is_running()
    }

    fn disk_stats(data: &mut RuntimeData, id: u32) -> u32 {
        let disk_id = id.try_into().unwrap_or_else(|_| abort("invalid disk id"));
        let stats = if let Some(disk) = data.emulator.disk(disk_id) {
            DiskStats {
                present: true,
                modified: disk.modified,
                idle_time: disk.idle_time.try_into().unwrap_or(u32::max_value()),
            }
        } else {
            DiskStats::missing()
        };

        stats.as_bits()
    }

    fn disk_buffer(data: &mut RuntimeData, id: u32) -> *mut u8 {
        let disk_id = id.try_into().unwrap_or_else(|_| abort("invalid disk id"));
        data.emulator.disk_slot(disk_id).as_mut_ptr()
    }

    fn insert_disk(data: &mut RuntimeData, id: u32) {
        let disk_id = id.try_into().unwrap_or_else(|_| abort("invalid disk id"));
        data.emulator.insert_disk(disk_id);
    }

    fn remove_disk(data: &mut RuntimeData, id: u32) {
        let disk_id = id.try_into().unwrap_or_else(|_| abort("invalid disk id"));
        data.emulator.remove_disk(disk_id);
    }

    fn key_up(data: &mut RuntimeData, key: u32) {
        data.emulator.key_up(key as u16);
    }

    fn key_down(data: &mut RuntimeData, key: u32) {
        data.emulator.key_down(key as u16);
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo<'_>) -> ! {
    unsafe {
        core::arch::wasm32::unreachable()
    }
}
