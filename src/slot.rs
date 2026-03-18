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

    pub(crate) unsafe fn set(&self, value: T) {
        let val_ptr = self.value.get();

        unsafe {
            *val_ptr = Some(value);
        }
    }

    pub(crate) unsafe fn take(&self) -> Option<T> {
        let val_ptr = self.value.get();

        unsafe { std::ptr::replace(val_ptr, None) }
    }
}

#[cfg(test)]
mod tests {
    use crate::slot::Slot;

    #[test]
    fn test() {
        let slot = Slot::new();

        unsafe {
            slot.set(5);

            let val = slot.take();
            assert_eq!(val, Some(5));
        }
    }
}
