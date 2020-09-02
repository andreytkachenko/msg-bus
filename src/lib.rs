mod envelop;
mod hash_map;
mod list;
mod untyped;

use core::future::Future;
use core::marker::PhantomData;
use crossbeam::queue::SegQueue;
use envelop::{BoxedEnvelop, Message};
use hash_map::HashMap as SimpleHashMap;
use list::List as AppendList;
use parking_lot::Mutex;
use sharded_slab::Slab;
use std::any::{Any, TypeId};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use untyped::{DynType, Untyped};

pub trait Handler<M: Message> {
    fn handle(&mut self, msg: &M, bus: &BusInner);

    fn is_ready(&self, ctx: &mut Context<'_>) -> bool {
        true
    }
}

trait UntypedReceiver {
    fn any(&self) -> &dyn Any;
    fn register(&self, index: usize, dtype: DynType);
    fn send(&self, msg: BoxedEnvelop);
    fn process(&self, ctx: &mut Context, bus: &BusInner);
}

pub struct Receiver<M: Message> {
    registry: Arc<Slab<Mutex<Untyped>>>,
    queue: SegQueue<M>,
    handlers: AppendList<(usize, DynType)>,
    waker: Mutex<Option<Waker>>,
}

impl<M: Message> Receiver<M> {
    pub fn new(registry: Arc<Slab<Mutex<Untyped>>>) -> Self {
        Self {
            registry,
            queue: SegQueue::new(),
            handlers: AppendList::new(),
            waker: Mutex::new(None),
        }
    }

    #[inline]
    pub(crate) fn enqueue(&self, msg: M) {
        self.queue.push(msg);
        self.waker.lock().take().map(|w| w.wake());
    }

    #[inline]
    pub(crate) fn enqueue_async(&self, msg: M) {
        unimplemented!()
    }
}

impl<M: Message> UntypedReceiver for Receiver<M> {
    #[inline]
    fn any(&self) -> &dyn Any {
        self
    }

    #[inline]
    fn register(&self, index: usize, dtype: DynType) {
        debug_assert!(self.registry.contains(index));

        self.handlers.append((index, dtype));
    }

    #[inline]
    fn send(&self, msg: BoxedEnvelop) {
        let msg = msg.downcast::<M>().unwrap();
        self.enqueue(*msg);
    }

    fn process(&self, ctx: &mut Context, bus: &BusInner) {
        if self.handlers.is_empty() {
            if !self.queue.is_empty() {
                println!("{} messages pending the handler", self.queue.len());
            }
            return;
        }

        while let Ok(msg) = self.queue.pop() {
            for (index, dtype) in self.handlers.iter() {
                let guard = self.registry.get(*index).unwrap();
                let mut handler = guard.lock();

                dtype.dyn_handler(&mut *handler).handle(&msg, bus);
            }
        }

        self.waker.lock().replace(ctx.waker().clone());
    }
}

struct Shim {
    inner: Box<dyn UntypedReceiver>,
}

impl Shim {
    #[inline]
    pub fn new<M: Message>(registry: Arc<Slab<Mutex<Untyped>>>) -> Self {
        Self {
            inner: Box::new(Receiver::<M>::new(registry)),
        }
    }

    #[inline]
    pub fn cast_to_module<M: Message>(&self) -> &Receiver<M> {
        self.inner.any().downcast_ref::<Receiver<M>>().unwrap()
    }

    #[inline]
    pub fn register(&self, index: usize, dtype: DynType) {
        self.inner.register(index, dtype);
    }

    #[inline]
    pub fn broadcast_dyn(&self, msg: BoxedEnvelop) {
        self.inner.send(msg);
    }

    #[inline]
    pub fn broadcast<M: Message>(&self, msg: M) {
        self.cast_to_module().enqueue_async(msg);
    }

    #[inline]
    pub fn broadcast_sync<M: Message>(&self, msg: M) {
        self.cast_to_module().enqueue(msg);
    }

    #[inline]
    pub fn process(&self, ctx: &mut Context, bus: &BusInner) {
        self.inner.process(ctx, bus);
    }
}

pub struct Entry<'a, T> {
    id: usize,
    registry: Arc<Slab<Mutex<Untyped>>>,
    map: &'a SimpleHashMap<Shim>,
    _m: PhantomData<T>,
}

impl<'a, T> Entry<'a, T> {
    #[inline]
    pub fn subscribe<M>(&mut self) -> &mut Self
    where
        M: Message + 'static,
        T: Handler<M> + 'static,
    {
        let tid = TypeId::of::<M>();
        if self.map.get(tid).is_none() {
            self.map.insert(tid, Shim::new::<M>(self.registry.clone()));
        }

        self.map
            .get(tid)
            .unwrap()
            .register(self.id, DynType::new::<M, T>());

        self
    }
}

pub struct BusInner {
    registry: Arc<Slab<Mutex<Untyped>>>,
    receivers: SimpleHashMap<Shim>,
}

impl BusInner {
    #[inline]
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Slab::new()),
            receivers: SimpleHashMap::new(),
        }
    }

    #[inline]
    pub fn register<T: Send + 'static>(&self, val: T) -> Entry<T> {
        let id = self.registry.insert(Mutex::new(Untyped::new(val))).unwrap();

        Entry {
            id,
            registry: self.registry.clone(),
            map: &self.receivers,
            _m: Default::default(),
        }
    }

    #[inline]
    pub async fn send_async<M: Message>(&self, msg: M) {
        match self.receivers.get(TypeId::of::<M>()) {
            Some(r) => r.broadcast(msg),
            None => return println!("Unhandled message {:?}", core::any::type_name::<M>()),
        }
    }
    
    #[inline]
    pub fn send<M: Message>(&self, msg: M) {
        match self.receivers.get(TypeId::of::<M>()) {
            Some(r) => r.broadcast_sync(msg),
            None => return println!("Unhandled message {:?}", core::any::type_name::<M>()),
        }
    }


    #[inline]
    fn poll(&self, ctx: &mut Context, bus: &BusInner) -> Poll<()> {
        for (_, shim) in self.receivers.iter() {
            let _ = shim.process(ctx, bus);
        }

        Poll::Pending
    }
}

pub struct BusPoller {
    inner: Arc<BusInner>,
}

impl Future for BusPoller {
    type Output = ();

    #[inline]
    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<()> {
        let this = self.get_mut();

        this.inner.poll(ctx, this.inner.as_ref())
    }
}

#[derive(Clone)]
pub struct Bus {
    inner: Arc<BusInner>,
}

impl core::ops::Deref for Bus {
    type Target = BusInner;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}

impl Bus {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(BusInner::new()),
        }
    }

    #[inline]
    pub fn register<T: Send + 'static>(&self, val: T) -> Entry<T> {
        self.inner.register(val)
    }

    #[inline]
    pub fn poller(&self) -> impl Future<Output = ()> {
        BusPoller {
            inner: self.inner.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_mbus() {}
}
