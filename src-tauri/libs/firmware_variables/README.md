# Firmware Variables Library

This library provides an interface for interacting with UEFI firmware variables on Windows systems. It allows you to get, set, and delete UEFI variables, manage boot entries, and handle device paths.

## Features

- Access and manipulate UEFI variables.
- Manage UEFI boot entries and boot order.
- Handle device paths in UEFI.
- Adjust process privileges for accessing UEFI variables.

## Usage

Here is a basic example of how to use the library:

```rust
use firmware_variables::{get_variable, set_variable, get_boot_order};

fn main() {
    // Example: Get a UEFI variable
    let (value, attributes) = get_variable("SomeVariable").expect("Failed to get variable");
    println!("Value: {:?}, Attributes: {:?}", value, attributes);

    // Example: Set a UEFI variable
    set_variable("SomeVariable", b"NewValue").expect("Failed to set variable");

    // Example: Get the boot order
    let boot_order = get_boot_order().expect("Failed to get boot order");
    println!("Boot Order: {:?}", boot_order);
}
```

## Contributing

Contributions are welcome! Please feel free to submit a pull request or open an issue for any enhancements or bug fixes.

## License

This project is licensed under the MIT License. See the LICENSE file for details.