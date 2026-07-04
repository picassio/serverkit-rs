"""Text and numeric formatting utilities."""


def format_bytes(n, precision=1, suffix_sep=' '):
    """Format a byte count as a human-readable string using 1024-based units.

    Args:
        n: Byte count. ``None`` or ``0`` renders as ``'0 B'`` (or ``'0B'`` when
           ``suffix_sep`` is empty).
        precision: Number of decimal places to show.
        suffix_sep: Separator between the numeric value and the unit.

    Returns:
        A human-readable string such as ``'1.5 KB'`` or ``'2.3GiB'``.

    Examples:
        >>> format_bytes(512)
        '512.0 B'
        >>> format_bytes(1536)
        '1.5 KB'
        >>> format_bytes(1073741824, suffix_sep='')
        '1.0GB'
    """
    if n is None:
        return ''
    n = float(n)
    if n == 0:
        return f'0{suffix_sep}B'
    for suffix in ('B', 'KB', 'MB', 'GB', 'TB'):
        if n < 1024:
            return f'{n:.{precision}f}{suffix_sep}{suffix}'
        n /= 1024
    return f'{n:.{precision}f}{suffix_sep}PB'
