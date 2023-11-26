use std::io;
use std::time::SystemTime;
use std::sync::OnceLock;

use headers::Header;

/// UTC offset to pass into `DavBuilder::autoindex` if you want the
/// directory index timestamps to be in another timezone than UTC.
///
/// You can get the offset from e.g. the `time` crate using
/// [`time::UtcTime::current_local_offset()`](https://docs.rs/time/latest/time/struct.UtcOffset.html#method.current_local_offset).
/// If you do, make sure to read about
/// [soundness](https://docs.rs/time/latest/time/util/local_offset/enum.Soundness.html#)
///
/// Example:
///
/// ```no_run
/// use webdav_handler::time::UtcOffset;
///
/// // Option<UtcTime>.
/// let utctime = time::UtcOffset::current_local_offset().map(UtcOffset::from).ok();
/// ```
///
#[derive(Clone, Copy, Debug)]
pub struct UtcOffset {
    hours:  i8,
    minutes: i8,
    seconds: i8,
}

impl UtcOffset {
    /// Create a new `UtcOffset`.
    pub fn new(hours: i8, minutes: i8, seconds: i8) -> Result<UtcOffset, io::Error> {
        ::time::UtcOffset::from_hms(hours, minutes, seconds)
            .map(|o| o.into())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
    }
}

impl From<::time::UtcOffset> for UtcOffset {
    fn from(offset: ::time::UtcOffset) -> UtcOffset {
        let (hours, minutes, seconds) = offset.as_hms();
        UtcOffset { hours, minutes, seconds }
    }
}

pub(crate) fn systemtime_to_httpdate(t: SystemTime) -> String {
    let d = headers::Date::from(t);
    let mut v = Vec::new();
    d.encode(&mut v);
    v[0].to_str().unwrap().to_owned()
}

pub(crate) fn systemtime_to_rfc3339(t: SystemTime) -> String {
    // 1996-12-19T16:39:57Z
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::from(t).format(&Rfc3339)
        .unwrap_or("1970-01-01T00:00:00Z".into())
}

pub(crate) fn systemtime_to_localtime(t: SystemTime, offset: Option<UtcOffset>) -> String {
    static FORMAT: OnceLock<time::format_description::OwnedFormatItem> = OnceLock::new();
    let format = FORMAT.get_or_init(|| {
        // 1996-12-19 17:39
        time::format_description::parse_owned::<2>("[year]-[month]-[day] [hour]:[minute]").unwrap()
    });

    let mut tm = time::OffsetDateTime::from(t);
    if let Some(off) = offset {
        let offset = time::UtcOffset::from_hms(off.hours, off.minutes, off.seconds).unwrap();
        tm = tm.to_offset(offset);
    }
    tm.format(&format).unwrap_or("1970-01-01T00:00:00Z".into())
}
