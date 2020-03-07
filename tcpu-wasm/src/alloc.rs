use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

const PAGE_SIZE: usize = 65536;

pub struct Box<T> {
    ptr: NonNull<T>,
}

unsafe impl<T: Send> Send for Box<T> {}
unsafe impl<T: Sync> Sync for Box<T> {}

impl<T> Box<T> {
    pub unsafe fn new_zeroed() -> Self {
        let layout = core::alloc::Layout::new::<T>();
        if layout.size() == 0 {
            return Box {
                ptr: NonNull::dangling(),
            };
        }
        let existing_pages = unsafe { core::arch::wasm32::memory_size(0) };
        let free_ptr = existing_pages * PAGE_SIZE;
        let box_ptr = (free_ptr + layout.align() - 1) / layout.align();
        let box_ptr = core::cmp::max(box_ptr, layout.align());
        let next_free_ptr = box_ptr + layout.size();
        let required_pages = (next_free_ptr - 1) / PAGE_SIZE + 1;
        let grow_by = required_pages - existing_pages;
        unsafe { core::arch::wasm32::memory_grow(0, grow_by) };
        Box {
            ptr: unsafe { NonNull::new_unchecked(box_ptr as *mut T) }
        }
    }
}

impl<T> Deref for Box<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T> DerefMut for Box<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut() }
    }
}

impl<T> Drop for Box<T> {
    fn drop(&mut self) {
        panic!("don't drop boxes");
    }
}
