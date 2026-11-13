//! PCI address syntax for user-selectable devices.
pub fn valid_pci_address(value: &str) -> bool {
    value.is_empty()
        || (value.len() == 12
            && value.bytes().enumerate().all(|(i, b)| match i {
                4 | 7 => b == b':',
                10 => b == b'.',
                _ => b.is_ascii_hexdigit(),
            })
            && value.as_bytes()[11] <= b'7')
}
