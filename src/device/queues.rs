use std::{cmp::min, ptr};

use ash::vk;

#[derive(Debug, Clone, Copy)]
pub struct QueueFamily {
  pub index: u32,
  pub queue_count: u32,
}

impl PartialEq for QueueFamily {
  fn eq(&self, other: &Self) -> bool {
    self.index == other.index
  }
}

#[derive(Debug)]
pub struct QueueFamilies {
  pub graphics: QueueFamily,
  pub compute: Option<QueueFamily>,
  pub transfer: Option<QueueFamily>,
}

impl QueueFamilies {
  pub const FAMILY_COUNT: usize = 3;

  pub fn get_from_physical_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
  ) -> Result<Self, ()> {
    let properties =
      unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

    let mut graphics = None;
    let mut compute = None; // non graphics
    let mut transfer = None; // non graphics and non compute
    for (i, props) in properties.into_iter().enumerate() {
      let family = Some(QueueFamily {
        index: i as u32,
        queue_count: props.queue_count,
      });

      if props.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
        if graphics.is_none() {
          graphics = family;
        }
      } else if props.queue_flags.contains(vk::QueueFlags::COMPUTE) {
        if compute.is_none() {
          compute = family;
        }
      } else if props.queue_flags.contains(vk::QueueFlags::TRANSFER) {
        if transfer.is_none() {
          transfer = family;
        }
      }
    }

    if graphics.is_none() {
      return Err(());
    }

    Ok(QueueFamilies {
      graphics: graphics.unwrap(),
      compute,
      transfer,
    })
  }
}

fn queue_create_info(
  index: u32,
  count: u32,
  priorities_ptr: *const f32,
) -> vk::DeviceQueueCreateInfo {
  vk::DeviceQueueCreateInfo {
    s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
    queue_family_index: index,
    queue_count: count,
    p_queue_priorities: priorities_ptr,
    p_next: ptr::null(),
    flags: vk::DeviceQueueCreateFlags::empty(),
  }
}

#[derive(Debug)]
pub struct Queues {
  pub graphics: vk::Queue,
  pub compute: vk::Queue,
  pub transfer: vk::Queue,
}

impl Queues {
  // mid priorities for all queues
  const PRIORITIES: [f32; QueueFamilies::FAMILY_COUNT] = [0.5; QueueFamilies::FAMILY_COUNT];

  pub fn get_queue_create_infos(queue_families: &QueueFamilies) -> Vec<vk::DeviceQueueCreateInfo> {
    let mut create_infos = Vec::with_capacity(QueueFamilies::FAMILY_COUNT);

    if let Some(family) = queue_families.compute {
      create_infos.push(queue_create_info(
        family.index,
        1,
        Self::PRIORITIES.as_ptr(),
      ));
    }
    if let Some(family) = queue_families.transfer {
      create_infos.push(queue_create_info(
        family.index,
        1,
        Self::PRIORITIES.as_ptr(),
      ));
    }

    // add graphics queues, these substitute for missing families
    create_infos.push(queue_create_info(
      queue_families.graphics.index,
      min(
        queue_families.graphics.queue_count,
        1 + [queue_families.compute, queue_families.transfer]
          .into_iter()
          .filter(|f| f.is_none())
          .count() as u32,
      ),
      Self::PRIORITIES.as_ptr(),
    ));

    create_infos
  }

  pub unsafe fn retrieve(device: &ash::Device, queue_families: &QueueFamilies) -> Queues {
    let mut graphics_i = 0;
    let mut get_next_graphics_queue = || {
      let queue = device.get_device_queue(queue_families.graphics.index, graphics_i);
      if graphics_i + 1 < queue_families.graphics.queue_count {
        graphics_i += 1;
      }
      queue
    };

    let graphics = get_next_graphics_queue();
    let compute = match &queue_families.compute {
      Some(family) => device.get_device_queue(family.index, 0),
      None => get_next_graphics_queue(),
    };
    let transfer = match &queue_families.transfer {
      Some(family) => device.get_device_queue(family.index, 0),
      None => get_next_graphics_queue(),
    };

    Queues {
      graphics,
      compute,
      transfer,
    }
  }
}
