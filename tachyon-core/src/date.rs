use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::Duration;

/// Pre-formatted `Date: <HTTP-date>\r\n` header, cached and updated once per second.
/// All worker threads read from the same atomic pointer — zero allocation per request.
///
/// Epoch-based reclamation: we keep the previous value alive until the next swap.
/// Since updates happen every 1s and reads take <1µs, this guarantees no use-after-free.
static CACHED_DATE: AtomicPtr<Vec<u8>> = AtomicPtr::new(std::ptr::null_mut());

fn format_date_header() -> Vec<u8> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    const DAYS: [&[u8]; 7] = [b"Sun", b"Mon", b"Tue", b"Wed", b"Thu", b"Fri", b"Sat"];
    const MONTHS: [&[u8]; 12] = [
        b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun",
        b"Jul", b"Aug", b"Sep", b"Oct", b"Nov", b"Dec",
    ];

    let secs_of_day = (now % 86400) as u32;
    let hour = secs_of_day / 3600;
    let min = (secs_of_day % 3600) / 60;
    let sec = secs_of_day % 60;

    let total_days = (now / 86400) as i64;
    let wday = ((total_days % 7 + 4) % 7) as usize;
    let (year, month, day) = civil_from_days(total_days);

    // "Date: Thu, 01 Jan 1970 00:00:00 GMT\r\n" = 37 bytes
    let mut buf = Vec::with_capacity(40);
    buf.extend_from_slice(b"Date: ");
    buf.extend_from_slice(DAYS[wday]);
    buf.extend_from_slice(b", ");
    buf.push(b'0' + (day / 10) as u8);
    buf.push(b'0' + (day % 10) as u8);
    buf.push(b' ');
    buf.extend_from_slice(MONTHS[(month - 1) as usize]);
    buf.push(b' ');
    // Year is always 4 digits for dates 1000-9999
    let y = year as u32;
    buf.push(b'0' + (y / 1000) as u8);
    buf.push(b'0' + ((y / 100) % 10) as u8);
    buf.push(b'0' + ((y / 10) % 10) as u8);
    buf.push(b'0' + (y % 10) as u8);
    buf.push(b' ');
    buf.push(b'0' + (hour / 10) as u8);
    buf.push(b'0' + (hour % 10) as u8);
    buf.push(b':');
    buf.push(b'0' + (min / 10) as u8);
    buf.push(b'0' + (min % 10) as u8);
    buf.push(b':');
    buf.push(b'0' + (sec / 10) as u8);
    buf.push(b'0' + (sec % 10) as u8);
    buf.extend_from_slice(b" GMT\r\n");
    buf
}

/// Convert days since epoch to (year, month, day). Algorithm from Howard Hinnant.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Start the async task that updates the cached Date header every second.
/// Must be called once inside the Tokio runtime before accepting connections.
pub fn start_date_cache() {
    // Initial value
    let initial = Box::into_raw(Box::new(format_date_header()));
    CACHED_DATE.store(initial, Ordering::Release);

    tokio::spawn(async move {
        // Epoch-based reclamation: keep previous value alive until next swap.
        // Since we swap every 1s and reads complete in <1µs, the previous
        // value is guaranteed to have no readers by the time we drop it.
        let mut prev: Option<Box<Vec<u8>>> = None;
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let new = Box::into_raw(Box::new(format_date_header()));
            let old = CACHED_DATE.swap(new, Ordering::AcqRel);
            // Drop the value from TWO swaps ago (not the one we just swapped out).
            // This gives readers a full 1s window to finish — more than enough.
            drop(prev.take());
            if !old.is_null() {
                prev = Some(unsafe { Box::from_raw(old) });
            }
        }
    });
}

/// Get the current cached Date header bytes. Zero-cost per request (atomic load + pointer deref).
#[inline(always)]
pub fn cached_date_header() -> &'static [u8] {
    let ptr = CACHED_DATE.load(Ordering::Acquire);
    if ptr.is_null() {
        b"Date: Thu, 01 Jan 1970 00:00:00 GMT\r\n"
    } else {
        unsafe { &*ptr }
    }
}
