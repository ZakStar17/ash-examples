use std::ptr;

use ash::vk;

use crate::{device::PhysicalDevice, IMAGE_FORMAT, IMAGE_HEIGHT, IMAGE_WIDTH};

pub struct Image {
  vk_img: vk::Image,
  pub memory: vk::DeviceMemory,
  pub memory_type_i: u32,
  pub memory_size: u64,
}

impl std::ops::Deref for Image {
  type Target = vk::Image;

  fn deref(&self) -> &Self::Target {
    &self.vk_img
  }
}

impl Image {
  pub fn new(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    tiling: vk::ImageTiling,
    usage: vk::ImageUsageFlags,
    required_memory_properties: vk::MemoryPropertyFlags,
    optional_memory_properties: vk::MemoryPropertyFlags,
  ) -> Self {
    log::debug!("Creating image");
    let vk_img = create_image(device, tiling, usage);

    log::debug!("Allocating memory for image");
    let (memory, memory_type_i, memory_size) = allocate_image_memory(
      device,
      physical_device,
      vk_img,
      required_memory_properties,
      optional_memory_properties,
    );

    log::debug!("Binding memory to image");
    unsafe {
      device
        .bind_image_memory(vk_img, memory, 0)
        .expect("Failed to bind memory to image")
    };

    Self {
      vk_img,
      memory,
      memory_type_i,
      memory_size,
    }
  }

  pub fn save_to_file<P>(&self, device: &ash::Device, physical_device: &PhysicalDevice, path: P)
  where
    P: AsRef<std::path::Path>,
  {
    // image memory needs to not be busy (getting used by device)

    let mem_type_flags = physical_device
      .get_memory_type(self.memory_type_i)
      .property_flags;

    assert!(mem_type_flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE));

    if !mem_type_flags.contains(vk::MemoryPropertyFlags::HOST_COHERENT) {
      // If the memory is not coherent, reading from it may give old results even if the GPU has
      // finished
      // Invalidate memory in order to make all previous change available to the host
      let host_img_memory_range = vk::MappedMemoryRange {
        s_type: vk::StructureType::MAPPED_MEMORY_RANGE,
        p_next: ptr::null(),
        memory: self.memory,
        offset: 0,
        size: self.memory_size,
      };
      log::debug!("Invalidating image memory");
      unsafe {
        device
          .invalidate_mapped_memory_ranges(&[host_img_memory_range])
          .expect("Failed to invalidate host image memory_ranges");
      }
    }

    // map entire memory
    let image_bytes = unsafe {
      log::debug!("Mapping image memory");
      let ptr = device
        .map_memory(
          self.memory,
          0,
          self.memory_size,
          vk::MemoryMapFlags::empty(),
        )
        .expect("Failed to map image memory") as *const u8;
      std::slice::from_raw_parts(ptr, self.memory_size as usize)
    };

    // read bytes and save to file
    log::debug!("Saving image");
    image::save_buffer(
      path,
      image_bytes,
      IMAGE_WIDTH,
      IMAGE_HEIGHT,
      ::image::ColorType::Rgba8,
    )
    .expect("Failed to save image");
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_image(self.vk_img, None);
    device.free_memory(self.memory, None);
  }
}

fn create_image(
  device: &ash::Device,
  tiling: vk::ImageTiling,
  usage: vk::ImageUsageFlags,
) -> vk::Image {
  // 1 color layer 2d image
  let create_info = vk::ImageCreateInfo {
    s_type: vk::StructureType::IMAGE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::ImageCreateFlags::empty(),
    image_type: vk::ImageType::TYPE_2D,
    format: IMAGE_FORMAT,
    extent: vk::Extent3D {
      width: IMAGE_WIDTH,
      height: IMAGE_HEIGHT,
      depth: 1,
    },
    mip_levels: 1,
    array_layers: 1,
    samples: vk::SampleCountFlags::TYPE_1,
    tiling,
    usage,
    sharing_mode: vk::SharingMode::EXCLUSIVE,
    queue_family_index_count: 0,
    p_queue_family_indices: ptr::null(), // ignored if sharing mode is exclusive
    initial_layout: vk::ImageLayout::UNDEFINED,
  };

  unsafe {
    device
      .create_image(&create_info, None)
      .expect("Failed to create image")
  }
}

// usually all images of similar type will use only one memory allocation
fn allocate_image_memory(
  device: &ash::Device,
  physical_device: &PhysicalDevice,
  image: vk::Image,
  required_memory_properties: vk::MemoryPropertyFlags,
  optional_memory_properties: vk::MemoryPropertyFlags,
) -> (vk::DeviceMemory, u32, u64) {
  let memory_requirements = unsafe { device.get_image_memory_requirements(image) };

  // in this case you can sub allocate multiple times for the image and individually manage each 
  // allocation
  if memory_requirements.size >= physical_device.get_max_memory_allocation_size() {
    panic!("Memory required to allocate an image ({}mb) is higher than the maximum allowed on the device ({}mb)", 
      memory_requirements.size / 1_000_000,
      physical_device.get_max_memory_allocation_size() / 1_000_000
    );
  }

  // Try to find optimal memory type
  // If it doesn't exist, try to find required memory type
  let memory_type = physical_device
    .find_optimal_memory_type(
      memory_requirements.memory_type_bits,
      required_memory_properties,
      optional_memory_properties,
    )
    .expect("Failed to find appropriate memory type for allocating an image");

  let heap_size = physical_device.get_memory_type_heap(memory_type).size;
  if memory_requirements.size >= heap_size {
    panic!("Memory required to allocate an image ({}mb) is higher than the size of the requested heap ({}mb)", 
      memory_requirements.size / 1_000_000,
      heap_size / 1_000_000
    );
  }

  let allocate_info = vk::MemoryAllocateInfo {
    s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
    p_next: ptr::null(),
    allocation_size: memory_requirements.size,
    memory_type_index: memory_type,
  };

  // Even if memory requirements are less than heap size and max memory allocation size, this
  // operation may still fail because of no available memory
  // There is no reliable a way to know beforehand if a allocate operation is going to succeed or
  // not, so handle errors accordingly
  // see https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/vkAllocateMemory.html
  let memory = unsafe {
    device
      .allocate_memory(&allocate_info, None)
      .expect("Failed to allocate memory for an image")
  };

  (memory, memory_type, memory_requirements.size)
}
