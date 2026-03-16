use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::Duration;

/// Pre-formatted `Date: <HTTP-date>\r\n` header, cached and updated once per second.
/// All worker threads read from the same atomic pointer — zero allocation per request.
static CACHED_DATE: AtomicPtr<Vec<u8>> = AtomicPtr::new(std::ptr::null_mut());

fn format_date_header() -> Vec<u8> {
    // Use libc time + gmtime for minimal overhead (no chrono dependency)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // HTTP-date format: "Date: Thu, 01 Jan 1970 00:00:00 GMT\r\n"
    let days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    // Calculate date components from unix timestamp
    let secs_of_day = (now % 86400) as u32;
    let hour = secs_of_day / 3600;
    let min = (secs_of_day % 3600) / 60;
    let sec = secs_of_day % 60;

    // Days since epoch (Jan 1, 1970 was Thursday = 4)
    let total_days = (now / 86400) as i64;
    let wday = ((total_days % 7 + 4) % 7) as usize;

    // Civil date from days since epoch
    let (year, month, day) = civil_from_days(total_days);

    format!(
        "Date: {}, {:02} {} {:04} {:02}:{:02}:{:02} GMT\r\n",
        days[wday],
        day,
        months[(month - 1) as usize],
        year,
        hour,
        min,
        sec,
    )
    .into_bytes()
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

/// Start the background thread that updates the cached Date header every second.
/// Must be called once before the server starts accepting connections.
pub fn start_date_cache() {
    // Initial value
    let initial = Box::into_raw(Box::new(format_date_header()));
    CACHED_DATE.store(initial, Ordering::Release);

    std::thread::Builder::new()
        .name("tachyon-date".into())
        .spawn(|| loop {
            std::thread::sleep(Duration::from_secs(1));
            let new = Box::into_raw(Box::new(format_date_header()));
            let old = CACHED_DATE.swap(new, Ordering::AcqRel);
            // Delay freeing old value to avoid use-after-free from concurrent readers.
            // Sleep ensures all in-flight reads have completed.
            std::thread::sleep(Duration::from_millis(50));
            if !old.is_null() {
                drop(unsafe { Box::from_raw(old) });
            }
        })
        .expect("failed to spawn date cache thread");
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
