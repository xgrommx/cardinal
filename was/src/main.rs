use cardinal_sdk::{
    fsevent::{EventFlag, EventStream, FsEvent},
    fsevent_sys::FSEventStreamEventId,
    utils::{dev_of_path, event_id_to_timestamp},
};
use crossbeam::channel::{Receiver, unbounded};
use std::time::Duration;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string()
    });
    let event_stream = spawn_event_watcher(path, 0);
    let mut history_done = false;
    let dev = dev_of_path(c"/").unwrap();
    let timezone = chrono::Local::now().timezone();
    loop {
        let events = if history_done {
            // If history is done, we try to drain the event stream with a timeout.
            event_stream.recv_timeout(Duration::from_secs_f32(0.5)).ok()
        } else {
            event_stream.recv().ok()
        };
        if let Some(events) = events {
            for event in events {
                if event.flag.contains(EventFlag::HistoryDone) {
                    history_done = true;
                } else {
                    let timestamp = event_id_to_timestamp(dev, event.id);
                    let time = chrono::DateTime::from_timestamp(timestamp, 0)
                        .unwrap()
                        .with_timezone(&timezone);
                    println!("{}, {:?}, {:?}", time.to_string(), event.path, event.flag);
                }
            }
        } else {
            break;
        }
    }
}

fn spawn_event_watcher(
    path: String,
    since_event_id: FSEventStreamEventId,
) -> Receiver<Vec<FsEvent>> {
    let (sender, receiver) = unbounded();
    std::thread::spawn(move || {
        EventStream::new(
            &[&path],
            since_event_id,
            0.1,
            Box::new(move |events| {
                sender.send(events).unwrap();
            }),
        )
        .block_on()
        .unwrap();
    });
    receiver
}
