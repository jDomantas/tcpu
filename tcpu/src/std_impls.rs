use super::{DISK_SIZE, MEMORY_SIZE, Storage};

impl Storage<[u8; DISK_SIZE]> for Box<[u8; DISK_SIZE]> {
    fn as_ref(&self) -> &[u8; DISK_SIZE] { self }
    fn as_mut(&mut self) -> &mut [u8; DISK_SIZE] { self }
}

impl Storage<[u8; MEMORY_SIZE]> for Box<[u8; MEMORY_SIZE]> {
    fn as_ref(&self) -> &[u8; MEMORY_SIZE] { self }
    fn as_mut(&mut self) -> &mut [u8; MEMORY_SIZE] { self }
}
