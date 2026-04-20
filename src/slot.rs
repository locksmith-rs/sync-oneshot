#[cfg(loom)]
use loom::cell::UnsafeCell;

#[cfg(not(loom))]
use std::cell::UnsafeCell;

pub(crate) struct Slot<T> {
    value: UnsafeCell<Option<T>>,
}

impl<T> Slot<T> {
    pub(crate) fn new() -> Self {
        Self {
            value: UnsafeCell::new(None),
        }
    }

    #[inline]
    pub(crate) unsafe fn set(&self, value: T) {
        let val_ptr = self.value.get();
        #[cfg(loom)]
        let val_ptr: *mut Option<T> = val_ptr.with(|ptr| ptr as *mut _);

        unsafe {
            *val_ptr = Some(value);
        }
    }

    #[inline]
    pub(crate) unsafe fn take(&self) -> Option<T> {
        let val_ptr = self.value.get();
        #[cfg(loom)]
        let val_ptr: *mut Option<T> = val_ptr.with(|ptr| ptr as *mut _);

        unsafe { std::ptr::replace(val_ptr, None) }
    }
}

#[cfg(test)]
mod tests {
    use crate::slot::Slot;

    #[test]
    fn test_slot() {
        let test_inner = || {
            let slot = Slot::new();

            unsafe {
                slot.set(5);

                let val = slot.take();
                assert_eq!(val, Some(5));
            }
        };

        #[cfg(loom)]
        loom::model(test_inner);

        #[cfg(not(loom))]
        test_inner();
    }
}
