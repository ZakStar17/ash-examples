use std::{cmp::min, ops::Deref, pin::Pin, ptr};

use ash::vk;

#[derive(Debug)]
pub struct QueueFamily {
  pub index: u32,
  pub queue_count: u32,
}

#[derive(Debug)]
pub struct QueueFamilies {
  pub compute: QueueFamily,
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

    let mut compute = None;
    let mut transfer = None;
    for (i, family) in properties.iter().enumerate() {
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

    if compute.is_none() {
      return Err(());
    }

    // commonly used
    let unique_indices = [compute.as_ref(), transfer.as_ref()]
      .into_iter()
      .filter_map(|opt| opt.map(|f| f.index))
      .collect();

    Ok(QueueFamilies {
      compute: compute.unwrap(),
      transfer,
      unique_indices,
    })
  }

  pub fn get_compute_index(&self) -> u32 {
    self.compute.index
  }

  pub fn get_transfer_index(&self) -> u32 {
    match self.transfer.as_ref() {
      Some(family) => family.index,
      None => self.compute.index,
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

fn get_queue_create_infos(
  families: &QueueFamilies,
  priorities_ptr: *const f32,
) -> Vec<vk::DeviceQueueCreateInfo> {
  let mut queues_create_infos = Vec::with_capacity(QueueFamilies::FAMILY_COUNT);

  // add optional queues
  for optional_family in [&families.transfer] {
    if let Some(family) = optional_family {
      queues_create_infos.push(get_queue_create_info(family.index, 1, priorities_ptr));
    }
  }

  // add graphics queues, these will substitute not available queues
  queues_create_infos.push(get_queue_create_info(
    families.compute.index,
    min(
      // always watch out for limits
      families.compute.queue_count,
      // request remaining needed queues
      (QueueFamilies::FAMILY_COUNT - queues_create_infos.len()) as u32,
    ),
    priorities_ptr,
  ));

  queues_create_infos
}

pub struct Queues {
  pub compute: vk::Queue,
  pub transfer: vk::Queue,
}

pub struct QueueCreateInfos {
  _priorities: Pin<Box<[f32; QueueFamilies::FAMILY_COUNT]>>,
  _priorities_ptr: *const f32,
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
    let priorities_ptr = priorities.as_ptr();
    let create_infos = get_queue_create_infos(&queue_families, priorities_ptr);

    QueueCreateInfos {
      _priorities: priorities,
      _priorities_ptr: priorities_ptr,
      create_infos,
    }
  }

  pub unsafe fn retrieve(device: &ash::Device, families: &QueueFamilies) -> Queues {
    let mut compute_i = 0;
    let mut get_next_compute_queue = || {
      let queue = device.get_device_queue(families.compute.index, compute_i);
      if compute_i < families.compute.queue_count {
        compute_i += 1;
      }
      queue
    };

    let compute = get_next_compute_queue();
    let transfer = match &families.transfer {
      Some(family) => device.get_device_queue(family.index, 0),
      None => get_next_compute_queue(),
    };

    Queues { compute, transfer }
  }
}
