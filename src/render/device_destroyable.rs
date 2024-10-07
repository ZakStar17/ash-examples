use ash::vk;

pub trait ManuallyDestroyed {
  unsafe fn destroy_self(&self);
}
pub trait DeviceManuallyDestroyed {
  unsafe fn destroy_self(&self, device: &ash::Device);
}

impl<T: ManuallyDestroyed> DeviceManuallyDestroyed for T {
  unsafe fn destroy_self(&self, _device: &ash::Device) {
    self.destroy_self();
  }
}

macro_rules! destroy {
  ($($obj:expr),+) => {
    {
      use crate::render::device_destroyable::ManuallyDestroyed;
      $(ManuallyDestroyed::destroy_self($obj);)+
    }
  };

  ($device:expr => $($obj:expr),+) => {
    {
      use crate::render::device_destroyable::DeviceManuallyDestroyed;
      $(DeviceManuallyDestroyed::destroy_self($obj, $device);)+
    }
  };
}
pub(crate) use destroy;

// fill_destroyable_array_with_expression!(device, <exp>, 3) transforms into [<exp>, <exp>, <exp>]
// If any <exp> returns an error, all previous <exp> results get destroyed with
// DeviceManuallyDestroyed::destroy_self()
//
// example:
//    Here "v" implements DeviceManuallyDestroyed
//    "f" is a function that returns Ok(v) the first time and Err(err) the second time it is called
//    ```
//      let a = fill_destroyable_array_with_expression!(device, f(), 3);
//    ```
//    a's final value is Err(err);
//    f got called two times;
//    v was created and then destroyed with the trait's function
//
macro_rules! fill_destroyable_array_with_expression {
  ($device:expr, $ex:expr, $arr_size:tt) => {{
    use crate::render::device_destroyable::DeviceManuallyDestroyed;
    use std::mem::MaybeUninit;

    let mut tmp: [MaybeUninit<_>; $arr_size] = unsafe { MaybeUninit::uninit().assume_init() };
    let mut i = 0;
    let mut last_error = Ok(());
    while i < $arr_size {
      let exp_result: Result<_, _> = $ex;
      match exp_result {
        Ok(v) => {
          tmp[i] = MaybeUninit::new(v);
        }
        Err(err) => {
          last_error = Err(err);
          break;
        }
      };
      i += 1;
    }

    if let Err(err) = last_error {
      for (j, item) in tmp.into_iter().enumerate() {
        if j >= i {
          break;
        }
        unsafe {
          DeviceManuallyDestroyed::destroy_self(&item.assume_init(), $device);
        }
      }
      Err(err)
    } else {
      Ok(unsafe { std::mem::transmute::<[MaybeUninit<_>; $arr_size], [_; $arr_size]>(tmp) })
    }
  }};
}
pub(crate) use fill_destroyable_array_with_expression;

// special case for buffers or other objects that implement Default
// mem::transmute doesn't work for generic const arrays (see https://github.com/rust-lang/rust/issues/61956)
macro_rules! fill_destroyable_array_with_expression_using_default {
  ($device:expr, $ex:expr, $arr_size:tt) => {{
    use crate::render::device_destroyable::DeviceManuallyDestroyed;

    let mut tmp: [_; $arr_size] = [Default::default(); $arr_size];
    let mut i = 0;
    let mut last_error = Ok(());
    while i < $arr_size {
      let exp_result: Result<_, _> = $ex;
      match exp_result {
        Ok(v) => {
          tmp[i] = v;
        }
        Err(err) => {
          last_error = Err(err);
          break;
        }
      };
      i += 1;
    }

    if let Err(err) = last_error {
      for (j, item) in tmp.into_iter().enumerate() {
        if j >= i {
          break;
        }
        unsafe {
          DeviceManuallyDestroyed::destroy_self(&item, $device);
        }
      }
      Err(err)
    } else {
      Ok(tmp)
    }
  }};
}
pub(crate) use fill_destroyable_array_with_expression_using_default;

// same as fill_destroyable_array_with_expression but with an iterator instead of a
// general expression
// each item in the iterator should return an Result where the value implements
// DeviceManuallyDestroyed
// iter remaining items count has to be less or equal to $arr_size
macro_rules! fill_destroyable_array_from_iter {
  ($device:tt, $iter:expr, $arr_size:tt) => {{
    let mut iter = $iter; // make sure $iter isn't creating new iterators every time
    crate::device_destroyable::fill_destroyable_array_with_expression!(
      $device,
      iter.next().unwrap(),
      $arr_size
    )
  }};
}
pub(crate) use fill_destroyable_array_from_iter;

macro_rules! fill_destroyable_array_from_iter_using_default {
  ($device:tt, $iter:expr, $arr_size:tt) => {{
    let mut iter = $iter; // make sure $iter isn't creating new iterators every time
    crate::render::device_destroyable::fill_destroyable_array_with_expression_using_default!(
      $device,
      iter.next().unwrap(),
      $arr_size
    )
  }};
}
pub(crate) use fill_destroyable_array_from_iter_using_default;

impl<T: DeviceManuallyDestroyed> DeviceManuallyDestroyed for [T] {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    for value in self.iter() {
      value.destroy_self(device);
    }
  }
}

impl<T: DeviceManuallyDestroyed> DeviceManuallyDestroyed for Box<[T]> {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    for value in self.iter() {
      value.destroy_self(device);
    }
  }
}

impl ManuallyDestroyed for ash::Instance {
  unsafe fn destroy_self(&self) {
    self.destroy_instance(None);
  }
}

impl ManuallyDestroyed for ash::Device {
  unsafe fn destroy_self(&self) {
    self.destroy_device(None);
  }
}

impl DeviceManuallyDestroyed for vk::Fence {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_fence(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::Semaphore {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_semaphore(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::Image {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_image(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::Buffer {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_buffer(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::DeviceMemory {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.free_memory(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::RenderPass {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_render_pass(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::PipelineCache {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_pipeline_cache(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::ImageView {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_image_view(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::Framebuffer {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_framebuffer(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::Pipeline {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_pipeline(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::CommandPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_command_pool(*self, None);
  }
}
