pub trait Destroyable {
  unsafe fn destroy_self(self: &Self);
}
pub trait DeviceDestroyable {
  unsafe fn destroy_self(self: &Self, device: &ash::Device);
}

impl<T: Destroyable> DeviceDestroyable for T {
  unsafe fn destroy_self(self: &Self, _device: &ash::Device) {
    self.destroy_self();
  }
}

impl Destroyable for ash::Instance {
  unsafe fn destroy_self(self: &Self) {
    self.destroy_instance(None);
  }
}

impl Destroyable for ash::Device {
  unsafe fn destroy_self(self: &Self) {
    self.destroy_device(None);
  }
}

#[macro_export]
macro_rules! destroy {
  ($($obj:expr),+) => {
    unsafe {
      use crate::device_destroyable::Destroyable;
      $(Destroyable::destroy_self($obj);)+
    }
  };

  ($device:expr => $($obj:expr),+) => {
    {
      unsafe {
        use crate::device_destroyable::DeviceDestroyable;
        $(DeviceDestroyable::destroy_self($obj, $device);)+
      }
    }
  };
}
