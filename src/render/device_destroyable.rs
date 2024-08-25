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
  ($device:tt, $ex:expr, $arr_size:tt) => {{
    use crate::render::device_destroyable::DeviceManuallyDestroyed;
    use std::mem::MaybeUninit;

    let mut tmp: [MaybeUninit<_>; $arr_size] = unsafe { MaybeUninit::uninit().assume_init() };
    let mut macro_res = Ok(());
    for i in 0..$arr_size {
      let exp_result: Result<_, _> = $ex;
      tmp[i] = match exp_result {
        Ok(v) => MaybeUninit::new(v),
        Err(err) => {
          for j in 0..i {
            unsafe {
              DeviceManuallyDestroyed::destroy_self(&tmp[j].assume_init(), $device);
            }
          }
          macro_res = Err(err);
          break;
        }
      };
    }
    macro_res.map(|_| unsafe { std::mem::transmute::<_, [_; $arr_size]>(tmp) })
  }};
}
pub(crate) use fill_destroyable_array_with_expression;

// same as fill_destroyable_array_with_expression but with an iterator instead of a
// general expression
// each item in the iterator should return an Result where the value implements
// DeviceManuallyDestroyed
// iter remaining items count has to be less or equal to $arr_size
macro_rules! fill_destroyable_array_from_iter {
  ($device:tt, $iter:expr, $arr_size:tt) => {{
    use crate::render::device_destroyable::DeviceManuallyDestroyed;
    use std::mem::MaybeUninit;

    let mut tmp: [MaybeUninit<_>; $arr_size] = unsafe { MaybeUninit::uninit().assume_init() };
    let mut macro_res = Ok(());
    let mut iter = $iter;
    for i in 0..$arr_size {
      tmp[i] = match iter.next().unwrap() {
        Ok(v) => MaybeUninit::new(v),
        Err(err) => {
          for j in 0..i {
            unsafe {
              DeviceManuallyDestroyed::destroy_self(&tmp[j].assume_init(), $device);
            }
          }
          macro_res = Err(err);
          break;
        }
      };
    }
    macro_res.map(|_| unsafe { std::mem::transmute::<_, [_; $arr_size]>(tmp) })
  }};
}
pub(crate) use fill_destroyable_array_from_iter;

impl<T: DeviceManuallyDestroyed> DeviceManuallyDestroyed for Box<[T]> {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    for value in self.iter() {
      value.destroy_self(device);
    }
  }
}

impl<T: DeviceManuallyDestroyed> DeviceManuallyDestroyed for [T] {
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
