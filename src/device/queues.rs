use std::{cmp::min, marker::PhantomData, ptr};

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
        #[allow(clippy::collapsible_if)]
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

    Ok(QueueFamilies {
      compute: compute.unwrap(),
      transfer,
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

fn queue_create_info<'a>(
  index: u32,
  count: u32,
  priorities_ptr: *const f32,
) -> vk::DeviceQueueCreateInfo<'a> {
  vk::DeviceQueueCreateInfo {
    s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
    queue_family_index: index,
    queue_count: count,
    p_queue_priorities: priorities_ptr,
    p_next: ptr::null(),
    flags: vk::DeviceQueueCreateFlags::empty(),
    _marker: PhantomData,
  }
}

#[derive(Debug)]
pub struct Queues {
  pub compute: vk::Queue,
  pub transfer: vk::Queue,
}

impl Queues {
  // mid priorities for all queues
  const QUEUE_PRIORITIES: [f32; QueueFamilies::FAMILY_COUNT] = [0.5; QueueFamilies::FAMILY_COUNT];

  pub fn get_queue_create_infos(queue_families: &QueueFamilies) -> Vec<vk::DeviceQueueCreateInfo> {
    let mut create_infos = Vec::with_capacity(QueueFamilies::FAMILY_COUNT);

    if let Some(family) = queue_families.transfer.as_ref() {
      create_infos.push(queue_create_info(
        family.index,
        1,
        Self::QUEUE_PRIORITIES.as_ptr(),
      ));
    }

    // add graphics queues, these substitute for missing families
    create_infos.push(queue_create_info(
      queue_families.get_compute_index(),
      min(
        queue_families.compute.queue_count,
        1 + if queue_families.transfer.is_none() {
          1
        } else {
          0
        },
      ),
      Self::QUEUE_PRIORITIES.as_ptr(),
    ));

    create_infos
  }

  pub unsafe fn retrieve(device: &ash::Device, queue_families: &QueueFamilies) -> Queues {
    let mut compute_i = 0;
    let mut get_next_compute_queue = || {
      let queue = device.get_device_queue(queue_families.compute.index, compute_i);
      if compute_i + 1 < queue_families.compute.queue_count {
        compute_i += 1;
      }
      queue
    };

    let compute = get_next_compute_queue();

    let transfer = match &queue_families.transfer {
      Some(family) => device.get_device_queue(family.index, 0),
      None => get_next_compute_queue(),
    };

    Queues { compute, transfer }
  }
}
