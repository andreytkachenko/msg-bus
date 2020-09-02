use msg_bus::{Bus, BusInner, Handler};
use std::sync::Arc;


struct TmpReceiver;
struct TmpReceiver2;

impl Handler<f32> for TmpReceiver {
    fn handle(&mut self, msg: &f32, bus: &BusInner) {
        bus.send(1u16);
        
        println!("---> f32 {}", msg);
    }
}

impl Handler<u16> for TmpReceiver {
    fn handle(&mut self, msg: &u16, bus: &BusInner) {
        bus.send(1u32);
        println!("---> u16 {}", msg);
    }
}


impl Handler<u32> for TmpReceiver {
    fn handle(&mut self, msg: &u32, bus: &BusInner) {
        bus.send(2i32);
        println!("---> u32 {}", msg);
    }
}

impl Handler<i32> for TmpReceiver {
    fn handle(&mut self, msg: &i32, bus: &BusInner) {
        bus.send(3i16);
        println!("---> i32 {}", msg);
    }
}

impl Handler<i16> for TmpReceiver {
    fn handle(&mut self, msg: &i16, bus: &BusInner) {
        println!("---> i16 {}", msg);
    }
}

impl Handler<i32> for TmpReceiver2 {
    fn handle(&mut self, msg: &i32, bus: &BusInner) {
        bus.send(3i16);
        println!("---> 2 i32 {}", msg);
    }
}

impl Handler<i16> for TmpReceiver2 {
    fn handle(&mut self, msg: &i16, bus: &BusInner) {
        println!("---> 2 i16 {}", msg);
    }
}

#[tokio::main]
async fn main() {
    let b = Bus::new();

    b.register(TmpReceiver)
        .subscribe::<f32>()
        .subscribe::<u16>()
        .subscribe::<u32>()
        .subscribe::<i32>()
        .subscribe::<i16>();
        
    b.register(TmpReceiver2)
        .subscribe::<i32>()
        .subscribe::<i16>();

    b.send(32f32);

    b.poller().await
}
