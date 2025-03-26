use fsevent_sys::{FSEventsGetCurrentEventId, FSEventsGetLastEventIdForDeviceBeforeTime};
use libc::dev_t;
use std::{ffi::CStr, mem::MaybeUninit};

pub fn current_timestamp() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

pub fn current_event_id() -> u64 {
    unsafe { FSEventsGetCurrentEventId() }
}

pub fn dev_of_path(path: &CStr) -> std::io::Result<dev_t> {
    let mut stat = MaybeUninit::uninit();
    let ret = unsafe { libc::lstat(path.as_ptr(), stat.as_mut_ptr()) };
    if ret != 0 {
        return Err(std::io::Error::from_raw_os_error(ret));
    }
    Ok(unsafe { stat.assume_init().st_dev })
}

pub fn event_id_to_timestamp(dev: dev_t, event_id: u64) -> i64 {
    let mut begin = 0i64;
    let mut end = current_timestamp();
    loop {
        let mid = (begin + end) / 2;
        let mid_event_id = unsafe { FSEventsGetLastEventIdForDeviceBeforeTime(dev, mid as f64) };
        if mid == begin || mid == end {
            return mid;
        }
        if mid_event_id < event_id {
            begin = mid
        } else if mid_event_id > event_id {
            end = mid
        } else {
            return mid;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_id_to_timestamp() {
        let event_id = 49841209;
        let dev = dev_of_path(c"/").unwrap();
        let timestamp = event_id_to_timestamp(dev, event_id);
        dbg!(
            time::OffsetDateTime::from_unix_timestamp(timestamp)
                .unwrap()
                .replace_offset(time::UtcOffset::from_hms(8, 0, 0).unwrap())
        );
    }
}
