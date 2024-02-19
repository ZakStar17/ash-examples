use std::{cmp::min, ops::Deref, pin::Pin, ptr};

use ash::vk;

use crate::render::initialization::Surface;

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
  pub presentation: QueueFamily,
  pub graphics: QueueFamily,
  pub compute: Option<QueueFamily>,
  pub transfer: Option<QueueFamily>,
}

impl QueueFamilies {
  pub const FAMILY_COUNT: usize = 4;

  pub fn get_from_physical_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    surface: &Surface,
  ) -> Result<Self, ()> {
    let properties =
      unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

    let mut presentation = None; // will try to be equal to graphics
    let mut graphics = None;
    let mut compute = None; // non graphics
    let mut transfer = None; // non graphics and non compute
    for (i, props) in properties.into_iter().enumerate() {
      let family = Some(QueueFamily {
        index: i as u32,
        queue_count: props.queue_count,
      });

      // set presentation to the first supported family
      if presentation.is_none() && unsafe { surface.supports_queue_family(physical_device, i) } {
        presentation = family;
      }

      if props.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
        // set graphics to the first supported family
        if graphics.is_none() {
          graphics = family;
        }

        // set presentation and graphics to the first family that supports both
        if presentation.is_some_and(|presentation_family| {
          presentation_family != family.unwrap()
            && unsafe { surface.supports_queue_family(physical_device, i) }
        }) {
          graphics = family;
          presentation = family;
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

    if presentation.is_none() || graphics.is_none() {
      return Err(());
    }

    Ok(QueueFamilies {
      presentation: presentation.unwrap(),
      graphics: graphics.unwrap(),
      compute,
      transfer,
    })
  }

  pub fn get_presentation_index(&self) -> u32 {
    self.presentation.index
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

  pub fn get_compute_index(&self) -> u32 {
    match self.compute.as_ref() {
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

#[derive(Debug)]
pub struct Queues {
  pub presentation: vk::Queue,
  pub graphics: vk::Queue,
  pub compute: vk::Queue,
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

    if let Some(family) = queue_families.compute {
      create_infos.push(get_queue_create_info(family.index, 1, priorities.as_ptr()));
    }
    if let Some(family) = queue_families.transfer {
      create_infos.push(get_queue_create_info(family.index, 1, priorities.as_ptr()));
    }

    if queue_families.presentation != queue_families.graphics {
      create_infos.push(get_queue_create_info(
        queue_families.get_presentation_index(),
        1,
        priorities.as_ptr(),
      ));
    }

    // add graphics queues, these substitute for missing families
    create_infos.push(get_queue_create_info(
      queue_families.get_graphics_index(),
      min(
        queue_families.graphics.queue_count,
        1 + [queue_families.compute, queue_families.transfer]
          .into_iter()
          .filter(|f| f.is_none())
          .count() as u32,
      ),
      priorities.as_ptr(),
    ));

    QueueCreateInfos {
      _priorities: priorities,
      create_infos,
    }
  }

  pub unsafe fn retrieve(device: &ash::Device, queue_families: &QueueFamilies) -> Queues {
    let mut graphics_i = 0;
    let mut get_next_graphics_queue = || {
      let queue = device.get_device_queue(queue_families.graphics.index, graphics_i);
      if graphics_i < queue_families.graphics.queue_count {
        graphics_i += 1;
      }
      queue
    };

    let graphics = get_next_graphics_queue();
    let presentation = if queue_families.presentation == queue_families.graphics {
      graphics
    } else {
      device.get_device_queue(queue_families.presentation.index, 0)
    };

    let compute = match &queue_families.compute {
      Some(family) => device.get_device_queue(family.index, 0),
      None => get_next_graphics_queue(),
    };
    let transfer = match &queue_families.transfer {
      Some(family) => device.get_device_queue(family.index, 0),
      None => get_next_graphics_queue(),
    };

    Queues {
      presentation,
      graphics,
      compute,
      transfer,
    }
  }
}
