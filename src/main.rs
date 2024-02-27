use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use can_config_rs::config::Type;
use can_socketcan_platform_rs::frame::CanFrame;
use gilrs::{Button, Event, Gilrs};

fn main() {
    let mut gilrs = Gilrs::new().unwrap();

    // Iterate over all connected gamepads
    for (_id, gamepad) in gilrs.gamepads() {
        println!("{} is {:?}", gamepad.name(), gamepad.power_info());
    }

    let left_trigger = Arc::new(Mutex::new(0f32));
    let right_trigger = Arc::new(Mutex::new(0f32));

    let left_trigger_ = left_trigger.clone();
    let right_trigger_ = right_trigger.clone();

    std::thread::spawn(move || {
        let config = can_live_config_rs::fetch_live_config().unwrap();
        let can_adapters: Vec<can_socketcan_platform_rs::CanSocket> = config
            .buses()
            .iter()
            .map(|bus_ref| can_socketcan_platform_rs::CanSocket::open(bus_ref.name()).unwrap())
            .collect();

        let node_names : Vec<&str> = config.nodes().iter().map(|n| n.name()).collect();
        println!("{node_names:?}");

        let node = config
            .nodes()
            .iter()
            .find(|n| n.name() == "gamepad")
            .expect("config doesn't define a node gamepad");
        let input_stream = node
            .tx_streams()
            .iter()
            .find(|s| s.name() == "input")
            .expect("gamepad doesn't define a tx_stream input");
        let input_message = input_stream.message();
        let input_adapter = &can_adapters[input_message.bus().id() as usize];

        let (ide, id) = match input_message.id() {
            can_config_rs::config::MessageId::StandardId(id) => (false, *id),
            can_config_rs::config::MessageId::ExtendedId(id) => (true, *id),
        };

        let interval = input_stream.min_interval();

        let left_trigger_ty = input_message
            .encoding()
            .unwrap()
            .attributes()
            .iter()
            .find(|att| att.name() == "left_trigger")
            .expect("input stream doesn't define a mapping for left_trigger")
            .ty();
        let right_trigger_ty = input_message
            .encoding()
            .unwrap()
            .attributes()
            .iter()
            .find(|att| att.name() == "right_trigger")
            .expect("input stream doesn't define a mapping for right_trigger")
            .ty();

        let can_config_rs::config::Type::Primitive(can_config_rs::config::SignalType::Decimal {
            size: left_size,
            offset: left_offset,
            scale: left_scale,
        }) = left_trigger_ty as &Type
        else {
            panic!("left trigger has invalid type expecting decimal");
        };

        let can_config_rs::config::Type::Primitive(can_config_rs::config::SignalType::Decimal {
            size: _right_size,
            offset: right_offset,
            scale: right_scale,
        }) = right_trigger_ty as &Type
        else {
            panic!("right trigger has invalid type expecting decimal");
        };

        // canzero
        loop {
            let left_trigger = *left_trigger_.lock().unwrap();
            let right_trigger = *right_trigger_.lock().unwrap();

            let data = (((left_trigger as f64 - *left_offset) / *left_scale) as u64) & 0xFFu64
                | ((((right_trigger as f64 - *right_offset) / *right_scale) as u64) & 0xFFu64)
                    << (*left_size as u32);
            let can_frame = CanFrame::new(id, ide, false, input_message.dlc(), data);
            // blocking
            input_adapter.transmit(&can_frame).unwrap();

            std::thread::sleep(Duration::from_millis(10));
        }
    });

    loop {
        // Examine new events
        while let Some(Event {
            id: _,
            event,
            time: _,
        }) = gilrs.next_event()
        {
            match event {
                gilrs::EventType::ButtonChanged(button, value, _) => match button {
                    Button::LeftTrigger2 => {
                        *left_trigger.lock().unwrap() = value;
                    }
                    Button::RightTrigger2 => {
                        *right_trigger.lock().unwrap() = value;
                    }
                    _ => (),
                },
                _ => (),
            }
        }
    }
}
