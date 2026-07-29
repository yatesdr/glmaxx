use std::sync::Arc;

use crate::{
    Fc1Descriptor, Fc2Descriptor, KernelError, validate_descriptor, validate_fc2_descriptor,
};

pub trait CudaDriver: Send + Sync + 'static {
    fn allocate(&self, bytes: u64, alignment: u64) -> Result<u64, KernelError>;
    fn free(&self, pointer: u64) -> Result<(), KernelError>;
    fn launch_fc1(&self, descriptor: &Fc1Descriptor, stream: u64) -> Result<(), KernelError>;
    fn launch_fc2(&self, descriptor: &Fc2Descriptor, stream: u64) -> Result<(), KernelError>;
    fn query_stream(&self, stream: u64) -> Result<bool, KernelError>;
}

pub struct Fc2LaunchTicket<D: CudaDriver> {
    driver: Arc<D>,
    stream: u64,
    sequence: u64,
}

impl<D: CudaDriver> Fc2LaunchTicket<D> {
    pub fn launch(
        driver: Arc<D>,
        descriptor: &Fc2Descriptor,
        stream: u64,
    ) -> Result<Self, KernelError> {
        validate_fc2_descriptor(descriptor)?;
        if stream == 0 {
            return Err(KernelError::Null);
        }
        driver.launch_fc2(descriptor, stream)?;
        Ok(Self {
            driver,
            stream,
            sequence: descriptor.sequence,
        })
    }

    pub fn is_complete(&self) -> Result<bool, KernelError> {
        self.driver.query_stream(self.stream)
    }

    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
}

pub struct DeviceBuffer<D: CudaDriver> {
    driver: Arc<D>,
    pointer: u64,
    bytes: u64,
}

impl<D: CudaDriver> DeviceBuffer<D> {
    pub fn allocate(driver: Arc<D>, bytes: u64, alignment: u64) -> Result<Self, KernelError> {
        if bytes == 0 || !alignment.is_power_of_two() {
            return Err(KernelError::Alignment);
        }
        let pointer = driver.allocate(bytes, alignment)?;
        if pointer == 0 || pointer % alignment != 0 {
            if pointer != 0 {
                let _ = driver.free(pointer);
            }
            return Err(KernelError::Alignment);
        }
        Ok(Self {
            driver,
            pointer,
            bytes,
        })
    }

    #[must_use]
    pub fn pointer(&self) -> u64 {
        self.pointer
    }

    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl<D: CudaDriver> Drop for DeviceBuffer<D> {
    fn drop(&mut self) {
        let _ = self.driver.free(self.pointer);
        self.pointer = 0;
    }
}

pub struct LaunchTicket<D: CudaDriver> {
    driver: Arc<D>,
    stream: u64,
    sequence: u64,
}

impl<D: CudaDriver> LaunchTicket<D> {
    pub fn launch(
        driver: Arc<D>,
        descriptor: &Fc1Descriptor,
        stream: u64,
    ) -> Result<Self, KernelError> {
        validate_descriptor(descriptor)?;
        if stream == 0 {
            return Err(KernelError::Null);
        }
        driver.launch_fc1(descriptor, stream)?;
        Ok(Self {
            driver,
            stream,
            sequence: descriptor.sequence,
        })
    }

    pub fn is_complete(&self) -> Result<bool, KernelError> {
        self.driver.query_stream(self.stream)
    }

    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct FakeDriver {
        freed: Mutex<Vec<u64>>,
    }

    impl CudaDriver for FakeDriver {
        fn allocate(&self, _bytes: u64, alignment: u64) -> Result<u64, KernelError> {
            Ok(alignment * 16)
        }

        fn free(&self, pointer: u64) -> Result<(), KernelError> {
            self.freed.lock().unwrap().push(pointer);
            Ok(())
        }

        fn launch_fc1(&self, _descriptor: &Fc1Descriptor, _stream: u64) -> Result<(), KernelError> {
            Ok(())
        }

        fn launch_fc2(&self, _descriptor: &Fc2Descriptor, _stream: u64) -> Result<(), KernelError> {
            Ok(())
        }

        fn query_stream(&self, _stream: u64) -> Result<bool, KernelError> {
            Ok(true)
        }
    }

    #[test]
    fn device_allocation_is_freed_exactly_once() {
        let driver = Arc::new(FakeDriver::default());
        {
            let buffer = DeviceBuffer::allocate(driver.clone(), 4096, 256).unwrap();
            assert_eq!(buffer.pointer(), 4096);
        }
        assert_eq!(*driver.freed.lock().unwrap(), [4096]);
    }
}
