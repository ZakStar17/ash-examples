use std::{cmp::min, marker::PhantomData, ptr};

use ash::vk;

use crate::render::initialization::{Surface, SurfaceError};

#[derive(Debug, Clone, Copy)]
pub struct QueueFamily {
  pub index: u32,
  pub queue_count: u32,
}

#[derive(Debug)]
pub struct QueueFamilies {
  pub graphics: QueueFamily,
  pub presentation: QueueFamily,
  pub transfer: Option<QueueFamily>,
}

#[derive(Debug, thiserror::Error)]
pub enum QueueFamilyError {
  #[error("Surface error")]
  SurfaceError(#[source] SurfaceError),
  #[error("Device does not support required queue families or surface capabilities")]
  DoesNotSupportRequiredQueueFamilies,
}

impl From<SurfaceError> for QueueFamilyError {
  fn from(value: SurfaceError) -> Self {
    QueueFamilyError::SurfaceError(value)
  }
}

impl QueueFamilies {
  pub const FAMILY_COUNT: usize = 3;

  pub fn get_from_physical_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    surface: &Surface,
  ) -> Result<Self, QueueFamilyError> {
    let properties =
      unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

    let mut presentation = None; // will try to be equal to graphics
    let mut graphics = None;
    let mut transfer = None; // non graphics
    for (i, props) in properties.into_iter().enumerate() {
      let family = Some(QueueFamily {
        index: i as u32,
        queue_count: props.queue_count,
      });

      // set presentation to the first supported family
      if presentation.is_none() && unsafe { surface.supports_queue_family(physical_device, i)? } {
        presentation = family;
      }

      if props.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
        // set graphics to the first supported family
        if graphics.is_none() {
          graphics = family;
        }

        // set presentation and graphics to the first family that supports both
        if let Some(presentation_family) = presentation.as_ref() {
          if presentation_family.index != family.as_ref().unwrap().index
            && unsafe { surface.supports_queue_family(physical_device, i) }?
          {
            graphics = family;
            presentation = family;
          }
        }
      } else if props.queue_flags.contains(vk::QueueFlags::TRANSFER) {
        if transfer.is_none() {
          transfer = family;
        }
      }
    }

    if presentation.is_none() || graphics.is_none() {
      return Err(QueueFamilyError::DoesNotSupportRequiredQueueFamilies);
    }

    Ok(QueueFamilies {
      graphics: graphics.unwrap(),
      presentation: presentation.unwrap(),
      transfer,
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

  pub fn get_presentation_index(&self) -> u32 {
    self.presentation.index
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
  pub graphics: vk::Queue,
  pub presentation: vk::Queue,
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

    if queue_families.presentation.index != queue_families.graphics.index {
      create_infos.push(queue_create_info(
        queue_families.get_presentation_index(),
        1,
        Self::QUEUE_PRIORITIES.as_ptr(),
      ));
    }

    // add graphics queues, these substitute for missing families
    // (presentation will try to be equal to graphics)
    create_infos.push(queue_create_info(
      queue_families.get_graphics_index(),
      min(
        queue_families.graphics.queue_count,
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
    let mut graphics_i = 0;
    let mut get_next_graphics_queue = || {
      let queue = device.get_device_queue(queue_families.graphics.index, graphics_i);
      if graphics_i + 1 < queue_families.graphics.queue_count {
        graphics_i += 1;
      }
      queue
    };

    let graphics = get_next_graphics_queue();
    let presentation = if queue_families.presentation.index == queue_families.graphics.index {
      graphics
    } else {
      device.get_device_queue(queue_families.presentation.index, 0)
    };

    let transfer = match &queue_families.transfer {
      Some(family) => device.get_device_queue(family.index, 0),
      None => get_next_graphics_queue(),
    };

    Queues {
      graphics,
      presentation,
      transfer,
    }
  }
}
