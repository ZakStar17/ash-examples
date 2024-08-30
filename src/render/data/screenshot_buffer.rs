use std::{
  marker::PhantomData,
  ptr::{self, NonNull},
};

use ash::vk;

use crate::{
  render::{
    allocator::{allocate_and_bind_memory, PackedAllocation},
    create_objs::create_buffer,
    device_destroyable::DeviceManuallyDestroyed,
    errors::{AllocationError, OutOfMemoryError},
    initialization::device::{Device, PhysicalDevice},
    IMAGE_WITH_RESOLUTION_MINIMAL_SIZE,
  },
  utility::OnErr,
};

use super::MappedHostBuffer;

pub struct ScreenshotBuffer {
  pub buffer: MappedHostBuffer<u8>,
  alloc: PackedAllocation,
}

impl ScreenshotBuffer {
  const PRIORITY: f32 = 0.2;
  const BUFFER_SIZE: u64 = IMAGE_WITH_RESOLUTION_MINIMAL_SIZE;

  pub fn new(device: &Device, physical_device: &PhysicalDevice) -> Result<Self, AllocationError> {
    let buffer = create_buffer(
      &device,
      Self::BUFFER_SIZE,
      vk::BufferUsageFlags::TRANSFER_DST,
    )?;

    let memory_requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let alloc = allocate_and_bind_memory(
      &device,
      &physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE,
      &[buffer],
      &[memory_requirements],
      &[],
      &[],
      Self::PRIORITY,
    )
    .on_err(|_| unsafe { buffer.destroy_self(device) })?;

    let data_ptr = unsafe {
      device.map_memory(
        alloc.memory,
        alloc.offsets.buffer_offsets()[0],
        // if size is not vk::WHOLE_SIZE, mapping should follow alignments
        vk::WHOLE_SIZE,
        vk::MemoryMapFlags::empty(),
      )
    }? as *mut u8;
    let data_ptr = NonNull::new(data_ptr).unwrap();

    Ok(Self {
      buffer: MappedHostBuffer { buffer, data_ptr },
      alloc,
    })
  }

  pub unsafe fn invalidate_memory(
    &self,
    device: &Device,
    physical_device: &PhysicalDevice,
  ) -> Result<(), OutOfMemoryError> {
    if !physical_device.mem_properties.memory_types[self.alloc.type_index as usize]
      .property_flags
      .contains(vk::MemoryPropertyFlags::HOST_COHERENT)
    {
      let range = vk::MappedMemoryRange {
        s_type: vk::StructureType::MAPPED_MEMORY_RANGE,
        p_next: ptr::null(),
        memory: self.alloc.memory,
        offset: self.alloc.offsets.buffer_offsets()[0],
        size: vk::WHOLE_SIZE,
        _marker: PhantomData,
      };
      device.invalidate_mapped_memory_ranges(&[range])?;
    }
    Ok(())
  }

  pub unsafe fn read_memory(&self) -> &[u8] {
    std::slice::from_raw_parts(self.buffer.data_ptr.as_ptr(), Self::BUFFER_SIZE as usize)
  }
}

impl DeviceManuallyDestroyed for ScreenshotBuffer {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.buffer.destroy_self(device);
    self.alloc.memory.destroy_self(device);
  }
}
