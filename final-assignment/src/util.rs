pub(crate) trait UncheckedAtomics {
    type Target;

    unsafe fn load_unchecked(&self) -> Self::Target {
        // SAFETY: There is no such thing as atomic data, just atomic operations.
        // Under the hood, data types like AtomicU32 are just u32 with a wrapper that only allows
        // for tomic operations.
        unsafe { core::ptr::read(self as *const Self as *const Self::Target) }
    }

    #[allow(dead_code)]
    unsafe fn store_unchecked(&self, value: Self::Target) {
        // SAFETY: There is no such thing as atomic data, just atomic operations.
        // Under the hood, data types like AtomicU32 are just u32 with a wrapper that only allows
        // for tomic operations. The allow(invalid_reference_casting) is because rust does have
        // correct semantics to represent this.
        #[allow(invalid_reference_casting)]
        unsafe {
            core::ptr::write(self as *const Self as *mut Self::Target, value)
        }
    }
}

impl UncheckedAtomics for core::sync::atomic::AtomicU32 {
    type Target = u32;
}

impl UncheckedAtomics for core::sync::atomic::AtomicU64 {
    type Target = u64;
}
