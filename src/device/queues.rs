use std::{cmp::min, ops::Deref, pin::Pin, ptr};

use ash::vk;

#[derive(Debug)]
pub struct QueueFamily {
  pub index: u32,
  pub queue_count: u32,
}

#[derive(Debug)]
pub struct QueueFamilies {
  pub graphics: QueueFamily,
  pub transfer: Option<QueueFamily>,
  pub unique_indices: Box<[u32]>,
}

impl QueueFamilies {
  pub const FAMILY_COUNT: usize = 2;

  pub fn get_from_physical_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
  ) -> Result<Self, ()> {
    let properties =
      unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

    let mut graphics = None;
    let mut compute = None;
    let mut transfer = None;
    for (i, family) in properties.iter().enumerate() {
      if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
        if graphics.is_none() {
          graphics = Some(QueueFamily {
            index: i as u32,
            queue_count: family.queue_count,
          });
        }
      }
      if family.queue_flags.contains(vk::QueueFlags::COMPUTE) {
        if compute.is_none() {
          compute = Some(QueueFamily {
            index: i as u32,
            queue_count: family.queue_count,
          });
        }
      } else if family.queue_flags.contains(vk::QueueFlags::TRANSFER) {
        if transfer.is_none() {
          transfer = Some(QueueFamily {
            index: i as u32,
            queue_count: family.queue_count,
          });
        }
      }
    }

    if graphics.is_none() {
      return Err(());
    }

    if transfer.is_none() && compute.is_some() {
      transfer = compute;
    }

    // commonly used
    let unique_indices = [graphics.as_ref(), transfer.as_ref()]
      .into_iter()
      .filter_map(|opt| opt.map(|f| f.index))
      .collect();

    Ok(QueueFamilies {
      graphics: graphics.unwrap(),
      transfer,
      unique_indices,
    })
  }

  pub fn get_graphics_index(&self) -> u32 {
    self.graphics.index
  }

  pub fn get_transfer_index(&self) -> u32 {
    match self.transfer.as_ref() {
      Some(family) => family.index,
      None => self.graphics.index,
    }
  }
}

fn get_queue_create_info(
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

pub struct Queues {
  pub graphics: vk::Queue,
  pub transfer: vk::Queue,
}

pub struct QueueCreateInfos {
  // create infos contains a ptr to priorities, so it has to own it as well
  _priorities: Pin<Box<[f32; QueueFamilies::FAMILY_COUNT]>>,
  create_infos: Vec<vk::DeviceQueueCreateInfo>,
}

impl Deref for QueueCreateInfos {
  type Target = Vec<vk::DeviceQueueCreateInfo>;

  fn deref(&self) -> &Self::Target {
    &self.create_infos
  }
}

impl Queues {
  pub fn get_queue_create_infos(queue_families: &QueueFamilies) -> QueueCreateInfos {
    // use mid priorities for all queues
    let priorities = Box::pin([0.5_f32; QueueFamilies::FAMILY_COUNT]);

    let mut create_infos = Vec::with_capacity(QueueFamilies::FAMILY_COUNT);

    // add optional queues
    for optional_family in [&queue_families.transfer] {
      if let Some(family) = optional_family {
        create_infos.push(get_queue_create_info(family.index, 1, priorities.as_ptr()));
      }
    }

    // add graphics queues, these will substitute not available queues
    create_infos.push(get_queue_create_info(
      queue_families.graphics.index,
      min(
        // always watch out for limits
        queue_families.graphics.queue_count,
        // request remaining needed queues
        (QueueFamilies::FAMILY_COUNT - create_infos.len()) as u32,
      ),
      priorities.as_ptr(),
    ));

    QueueCreateInfos {
      _priorities: priorities,
      create_infos,
    }
  }

  pub unsafe fn retrieve(device: &ash::Device, families: &QueueFamilies) -> Queues {
    //! Should match order in get_queue_create_infos exactly

    let mut graphics_i = 0;
    let mut get_next_graphics_queue = || {
      let queue = device.get_device_queue(families.graphics.index, graphics_i);
      if graphics_i < families.graphics.queue_count {
        graphics_i += 1;
      }
      queue
    };

    let graphics = get_next_graphics_queue();
    let transfer = match &families.transfer {
      Some(family) => device.get_device_queue(family.index, 0),
      None => get_next_graphics_queue(),
    };

    Queues { graphics, transfer }
  }
}
