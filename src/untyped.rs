use core::any::TypeId;

use crate::{Message, Handler};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TraitObject {
    pub data: *mut (),
    pub vtable: *mut (),
}

pub struct Untyped {
    data: *mut (),
    type_id: TypeId,
    size: usize,
}

unsafe impl Send for Untyped {}

impl Untyped {
    pub fn new<T: Send + 'static>(val: T) -> Self {
        Self {
            data: Box::into_raw(Box::new(val)) as *mut (),
            type_id: TypeId::of::<T>(),
            size: std::mem::size_of::<T>(),
        }
    }
}

impl Drop for Untyped {
    fn drop(&mut self) {
        drop(unsafe { Vec::from_raw_parts(self.data, self.size, self.size) });
    }
}

pub struct DynType {
    msg_type_id: TypeId,
    type_id: TypeId,
    vtable: *mut (),
}

impl DynType {
    pub fn new<M: Message + 'static, T: Handler<M> + 'static>() -> Self {
        let to: TraitObject =
            unsafe { std::mem::transmute(&*(std::ptr::null() as *const T) as &dyn Handler<M>) };

        Self {
            msg_type_id: TypeId::of::<M>(),
            type_id: TypeId::of::<T>(),
            vtable: to.vtable,
        }
    }

    pub fn dyn_handler<'a, M: 'static>(
        &self,
        untyped: &'a mut Untyped,
    ) -> &'a mut (dyn Handler<M> + 'a) {
        assert_eq!(self.type_id, untyped.type_id);
        assert_eq!(self.msg_type_id, TypeId::of::<M>());

        let to = TraitObject {
            data: untyped.data,
            vtable: self.vtable,
        };

        unsafe { std::mem::transmute(to) }
    }
}

unsafe impl Send for DynType {}
unsafe impl Sync for DynType {}
