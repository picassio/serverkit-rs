"""SystemService.get_server_time must only ever expose an IANA timezone_id.

Regression for a Windows-dev crash: Python's time.tzname yields a platform
*display* name ("Eastern Daylight Time") which is not a valid IANA/Intl zone key
and crashed the panel's time-formatting. timezone_id must be IANA-or-null; the
human display name lives in timezone_name.
"""
from app.services.system_service import SystemService


def test_is_iana_timezone():
    assert SystemService._is_iana_timezone('America/New_York') is True
    assert SystemService._is_iana_timezone('Europe/London') is True
    assert SystemService._is_iana_timezone('UTC') is True
    # Windows display names are rejected.
    assert SystemService._is_iana_timezone('Eastern Daylight Time') is False
    assert SystemService._is_iana_timezone('Pacific Standard Time') is False
    assert SystemService._is_iana_timezone('') is False
    assert SystemService._is_iana_timezone(None) is False


def test_get_server_time_timezone_id_is_iana_or_null():
    t = SystemService.get_server_time()
    tzid = t.get('timezone_id')
    # Never a spaced display name; either a real IANA key or null.
    assert tzid is None or SystemService._is_iana_timezone(tzid), tzid
    # The display name is still available separately for the UI clock.
    assert 'timezone_name' in t


def test_get_server_time_shape(app):
    t = SystemService.get_server_time()
    for key in ('current_time', 'current_time_formatted', 'utc_time',
                'timezone_name', 'timezone_id', 'utc_offset'):
        assert key in t
